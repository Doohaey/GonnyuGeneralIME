// build.rs — Encrypts resources/ and embeds them as Rust constants for the FFI crate.
// At compile time: walks resources/, encrypts each file with XChaCha20-Poly1305,
// and generates embedded_resources.rs in OUT_DIR.
//
// Output format per file: magic "GNYE"(4) + 种子(24) + 密文
//
// Key management (whitebox):
//   * The 32-byte key is NOT hardcoded in this repository. It is supplied at
//     build time via the GANNYU_RESOURCE_KEY environment variable (64 hex chars).
//     CI injects it from GitHub Secrets; local/dev builds fall back to a random
//     per-build key.
//   * The key is never embedded directly. Instead, a per-build S-box and byte
//     permutation are generated, and the key is run through a Feistel network
//     to produce an "embedded seed". Only the seed (XOR-masked and interleaved
//     with garbage) plus the S-box/permutation constants are embedded.
//   * At runtime, `whitebox::derive_master_key()` inverts the Feistel to
//     recover the key. Recovering it statically requires locating the
//     interleaved constants and reversing the Feistel rounds.
//   * This layer raises the cost of extracting the dictionary resources; it is
//     NOT a hard security boundary — a determined attacker can still recover
//     the key from the binary at runtime.

use chacha20poly1305::aead::{Aead, KeyInit, OsRng};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use hmac::{Hmac, Mac};
use rand::RngCore;
use sha2::Sha256;
use std::env;
use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

type HmacSha256 = Hmac<Sha256>;

const 文件头标记: &[u8; 4] = b"GNYE";

/// Number of Feistel rounds. Must match `whitebox.rs`.
const ROUNDS: usize = 10;

/// Resolve the 32-byte resource key.
///
/// Priority:
///   1. `GANNYU_RESOURCE_KEY` env var (64 hex chars) — production/CI.
///   2. Random per-build key — local/dev fallback.
fn resolve_key() -> [u8; 32] {
    let mut key = [0u8; 32];
    if let Ok(hex) = env::var("GANNYU_RESOURCE_KEY") {
        let hex = hex.trim();
        if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
            for (i, byte) in key.iter_mut().enumerate() {
                *byte = u8::from_str_radix(&hex[i * 2..i * 2 + 2], 16).unwrap();
            }
        } else {
            panic!("GANNYU_RESOURCE_KEY must be exactly 64 hex characters");
        }
    } else if env::var("PROFILE").as_deref() == Ok("release") {
        panic!("GANNYU_RESOURCE_KEY is required for release builds");
    } else {
        OsRng.fill_bytes(&mut key);
        println!("cargo:warning=using an ephemeral resource key for a development build");
    }
    key
}

/// Generate a random S-box: a permutation of 0..255.
fn generate_sbox() -> [u8; 256] {
    let mut sbox = [0u8; 256];
    for (i, v) in sbox.iter_mut().enumerate() {
        *v = i as u8;
    }
    // Fisher-Yates shuffle.
    for i in (1..256).rev() {
        let j = (OsRng.next_u32() as usize) % (i + 1);
        sbox.swap(i, j);
    }
    sbox
}

/// Generate a random byte permutation of 0..31.
fn generate_perm() -> [u8; 32] {
    let mut perm = [0u8; 32];
    for (i, v) in perm.iter_mut().enumerate() {
        *v = i as u8;
    }
    for i in (1..32).rev() {
        let j = (OsRng.next_u32() as usize) % (i + 1);
        perm.swap(i, j);
    }
    perm
}

/// The Feistel round function. Must match `whitebox.rs`.
fn round_function(right: &[u8; 16], round: usize, sbox: &[u8; 256]) -> [u8; 16] {
    let mut out = [0u8; 16];
    for i in 0..16 {
        let x = right[i];
        let s = sbox[x as usize];
        let mixed = s
            .wrapping_add((round as u8).wrapping_mul(0x2f))
            .wrapping_add(0x53)
            .wrapping_add(right[(i + 5) % 16]);
        out[i] = sbox[mixed as usize];
    }
    out
}

/// Apply the byte permutation to a 32-byte block. Must match `whitebox.rs`.
fn permute(block: &mut [u8; 32], perm: &[u8; 32]) {
    let mut tmp = [0u8; 32];
    for i in 0..32 {
        tmp[i] = block[perm[i] as usize];
    }
    *block = tmp;
}

/// Forward Feistel: `embedded = feistel(key)`. Must match `whitebox.rs`.
fn feistel_forward(block: &mut [u8; 32], sbox: &[u8; 256], perm: &[u8; 32]) {
    for round in 0..ROUNDS {
        let mut left = [0u8; 16];
        let mut right = [0u8; 16];
        left.copy_from_slice(&block[0..16]);
        right.copy_from_slice(&block[16..32]);
        let f = round_function(&right, round, sbox);
        let new_left = right;
        let mut new_right = [0u8; 16];
        for i in 0..16 {
            new_right[i] = left[i] ^ f[i];
        }
        block[0..16].copy_from_slice(&new_left);
        block[16..32].copy_from_slice(&new_right);
        permute(block, perm);
    }
}

/// Interleave a byte array with garbage at odd indices.
///
/// The garbage byte is derived from the real byte via a per-fragment affine
/// transform `b * mult + add` (mod 256). Varying `mult`/`add` per fragment
/// defeats a uniform signature scan (previously every odd byte satisfied
/// `odd = even*0x9e + 0x37`, which let an attacker locate the whole blob in
/// one pass).
fn interleave(data: &[u8], mult: u8, add: u8) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len() * 2);
    for &b in data {
        out.push(b);
        out.push(b.wrapping_mul(mult).wrapping_add(add));
    }
    out
}

/// A single scattered fragment of the whitebox constants.
struct Fragment {
    /// Byte offset of this fragment within the logical (de-interleaved) blob.
    offset: usize,
    /// Number of real bytes in this fragment.
    len: usize,
    /// Per-fragment XOR mask applied to the real bytes.
    mask: Vec<u8>,
    /// Garbage affine params for this fragment's interleave.
    mult: u8,
    add: u8,
}

/// Split `data` (the logical, de-interleaved blob) into `n` fragments of
/// roughly equal size, each with its own XOR mask and garbage params.
fn split_fragments(data: &[u8], n: usize) -> Vec<Fragment> {
    let mut frags = Vec::with_capacity(n);
    let base = data.len() / n;
    let rem = data.len() % n;
    let mut offset = 0usize;
    for i in 0..n {
        let len = base + if i < rem { 1 } else { 0 };
        let mut mask = vec![0u8; len];
        OsRng.fill_bytes(&mut mask);
        // Per-fragment garbage params; ensure mult is odd so the affine map is
        // a bijection (invertible), avoiding degenerate constant garbage.
        let mult = (OsRng.next_u32() as u8) | 1;
        let add = OsRng.next_u32() as u8;
        frags.push(Fragment {
            offset,
            len,
            mask,
            mult,
            add,
        });
        offset += len;
    }
    frags
}

/// Emit a Rust array literal from bytes.
fn rust_array(bytes: &[u8]) -> String {
    let mut s = String::new();
    for chunk in bytes.chunks(8) {
        let parts: Vec<String> = chunk.iter().map(|b| format!("0x{b:02x}")).collect();
        s.push_str(&format!("    {},\n", parts.join(", ")));
    }
    s
}

fn encrypt_file(加密器: &XChaCha20Poly1305, path: &Path) -> Vec<u8> {
    let mut plaintext = Vec::new();
    File::open(path)
        .unwrap()
        .read_to_end(&mut plaintext)
        .unwrap();

    let mut 种子字节 = [0u8; 24];
    OsRng.fill_bytes(&mut 种子字节);
    let 种子 = XNonce::from_slice(&种子字节);

    let 密文 = 加密器.encrypt(种子, plaintext.as_ref()).unwrap();

    let mut out = Vec::with_capacity(4 + 24 + 密文.len());
    out.extend_from_slice(文件头标记);
    out.extend_from_slice(&种子字节);
    out.extend_from_slice(&密文);
    out
}

fn walk_resources(
    base: &Path,
    dir: &Path,
    加密器: &XChaCha20Poly1305,
    entries: &mut Vec<(String, Vec<u8>)>,
) {
    for entry in fs::read_dir(dir).unwrap() {
        let entry = entry.unwrap();
        let path = entry.path();
        let relative = path
            .strip_prefix(base)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/");

        // Skip build-time-only files: documentation, history data, and raw
        // source imports that are only kept for local maintenance.
        if relative.ends_with(".md")
            || relative.starts_with("regions/")
                && (relative.contains("/history/") || relative.contains("/raw/"))
        {
            continue;
        }

        if path.is_dir() {
            walk_resources(base, &path, 加密器, entries);
        } else {
            let encrypted = encrypt_file(加密器, &path);
            entries.push((relative, encrypted));
        }
    }
}

fn main() {
    let manifest_dir_val = env::var("CARGO_MANIFEST_DIR").unwrap();
    let manifest_dir = Path::new(&manifest_dir_val);
    let resources = env::var_os("GONNYU_RESOURCE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| manifest_dir.join("../../resources"));
    let out_dir_val = env::var("OUT_DIR").unwrap();
    let out_dir = Path::new(&out_dir_val);

    if !resources.is_dir() {
        // resources/ not found — generate empty module
        let code =
            "pub fn embedded_resources() -> Vec<(&'static str, &'static [u8])> { Vec::new() }\n";
        fs::write(out_dir.join("embedded_resources.rs"), code).unwrap();
        return;
    }

    let 密钥 = resolve_key();

    // Whitebox key derivation: generate a per-build S-box and permutation,
    // then embed the Feistel output of the key (masked + interleaved) plus the
    // S-box/permutation constants. The raw key never appears in the binary.
    let sbox = generate_sbox();
    let perm = generate_perm();

    // Mask the seed with a fixed mask so it is not stored in the clear.
    const 掩码: [u8; 32] = [
        0x5a, 0x3c, 0x91, 0x2e, 0x77, 0x48, 0x0d, 0x6f, 0x1b, 0xc4, 0x8e, 0x52, 0x39, 0xa7, 0x60,
        0x4d, 0x2b, 0x85, 0x1e, 0x73, 0xca, 0x09, 0x64, 0xbf, 0x41, 0x9d, 0x37, 0xec, 0x58, 0x12,
        0xa0, 0x7e,
    ];

    // embedded = feistel(key)
    let mut embedded = 密钥;
    feistel_forward(&mut embedded, &sbox, &perm);
    let mut masked_seed = [0u8; 32];
    for i in 0..32 {
        masked_seed[i] = embedded[i] ^ 掩码[i];
    }

    // Generate scattered whitebox fragments.
    //
    // Previously the whole set of constants was emitted as one contiguous
    // 704-byte blob (`spn_tables.bin`) with a uniform garbage signature, which
    // let an attacker locate it in a single pass and reverse the Feistel to
    // recover the key. Now the logical blob is split into several fragments,
    // each written as its own `.bin` and included via a separate
    // `include_bytes!`, so they land in different `.rodata` locations rather
    // than one contiguous run. Each fragment additionally has its own XOR mask
    // and its own garbage affine params, defeating both the contiguous-blob
    // scan and the uniform-signature scan.
    //
    // Logical (de-interleaved) layout:
    //   [sbox(256)][perm(32)][masked_seed(32)][mask(32)]  = 352 real bytes
    // Each real byte is interleaved with a garbage byte, so the on-disk size
    // is 704 bytes total across all fragments.
    let mut logical = Vec::with_capacity(256 + 32 + 32 + 32);
    logical.extend_from_slice(&sbox);
    logical.extend_from_slice(&perm);
    logical.extend_from_slice(&masked_seed);
    logical.extend_from_slice(&掩码);

    // Split into 8 fragments so no single contiguous run holds the whole set.
    let fragments = split_fragments(&logical, 8);

    // Emit a Rust source describing the fragments (offsets, masks, garbage
    // params) plus the include_bytes! references. The masks/params are emitted
    // as opaque byte arrays so the optimizer cannot constant-fold the
    // de-interleaving into a contiguous key/mask constant.
    let mut frag_code = String::new();
    frag_code.push_str("#[allow(clippy::all)]\n");
    frag_code.push_str("pub struct SpnFragment {\n");
    frag_code.push_str("    pub offset: usize,\n");
    frag_code.push_str("    pub len: usize,\n");
    frag_code.push_str("    pub mask: &'static [u8],\n");
    frag_code.push_str("    pub mult: u8,\n");
    frag_code.push_str("    pub add: u8,\n");
    frag_code.push_str("    pub data: &'static [u8],\n");
    frag_code.push_str("}\n");
    frag_code.push_str("#[allow(clippy::all)]\n");
    frag_code.push_str("pub const SPN_FRAGMENTS: &[SpnFragment] = &[\n");

    for (i, frag) in fragments.iter().enumerate() {
        // Build the on-disk fragment: real bytes XOR-masked, then interleaved
        // with per-fragment garbage.
        let mut masked = Vec::with_capacity(frag.len);
        for (j, &b) in logical[frag.offset..frag.offset + frag.len]
            .iter()
            .enumerate()
        {
            masked.push(b ^ frag.mask[j]);
        }
        let on_disk = interleave(&masked, frag.mult, frag.add);
        let frag_path = out_dir.join(format!("spn_frag_{i}.bin"));
        fs::write(&frag_path, &on_disk).unwrap();

        let mask_lit = rust_array(&frag.mask);
        frag_code.push_str(&format!(
            "    SpnFragment {{ offset: {}, len: {}, mult: 0x{:02x}, add: 0x{:02x}, mask: &[\n{}\n    ], data: include_bytes!(\"{}\") }},\n",
            frag.offset,
            frag.len,
            frag.mult,
            frag.add,
            mask_lit,
            frag_path.to_string_lossy().replace('\\', "/")
        ));
    }
    frag_code.push_str("];\n");
    fs::write(out_dir.join("spn_fragments.rs"), frag_code).unwrap();
    println!("cargo:rerun-if-env-changed=GANNYU_RESOURCE_KEY");
    println!("cargo:rerun-if-env-changed=GONNYU_RESOURCE_DIR");

    let 加密器 = XChaCha20Poly1305::new_from_slice(&密钥).unwrap();
    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    walk_resources(&resources, &resources, &加密器, &mut entries);

    // Whole-set integrity HMAC: bind all encrypted blobs together so that
    // swapping or modifying any blob is detected at runtime. The integrity key
    // is derived from the master key (never stored directly).
    let mut integrity_key = [0u8; 32];
    for i in 0..32 {
        integrity_key[i] = 密钥[i] ^ 0xa5;
    }
    let mut mac = <HmacSha256 as Mac>::new_from_slice(&integrity_key).unwrap();
    for (_, data) in &entries {
        mac.update(data);
    }
    let integrity_tag = mac.finalize().into_bytes();
    let mut integrity_code = String::new();
    integrity_code.push_str("#[allow(clippy::all)]\n");
    integrity_code.push_str("pub const RESOURCE_INTEGRITY_TAG: [u8; 32] = [\n");
    integrity_code.push_str(&rust_array(&integrity_tag));
    integrity_code.push_str("];\n");
    fs::write(out_dir.join("integrity.rs"), integrity_code).unwrap();

    // Build the Rust source code as a match function.
    //
    // Resource paths are XOR-obfuscated at build time so they do not appear as
    // contiguous plaintext strings in the binary. A per-build XOR key is
    // embedded (itself obfuscated); at runtime the paths are decoded once and
    // cached. This prevents `strings`/static scans from trivially listing the
    // embedded resource layout.
    let mut code = String::new();
    code.push_str("#[allow(clippy::all)]\n");

    // Per-build XOR key for path obfuscation. Stored as a byte array so it does
    // not appear as a contiguous ASCII string.
    let mut path_key = [0u8; 16];
    OsRng.fill_bytes(&mut path_key);
    code.push_str(&format!(
        "const PATH_XOR_KEY: [u8; 16] = [\n{}\n];\n",
        rust_array(&path_key)
    ));

    code.push_str("#[allow(clippy::all)]\n");
    code.push_str("fn decode_path(enc: &[u8]) -> String {\n");
    code.push_str("    let mut out = String::with_capacity(enc.len());\n");
    code.push_str("    for (i, &b) in enc.iter().enumerate() {\n");
    code.push_str("        out.push((b ^ PATH_XOR_KEY[i % PATH_XOR_KEY.len()]) as char);\n");
    code.push_str("    }\n");
    code.push_str("    out\n");
    code.push_str("}\n");

    // Encoded path constants.
    code.push_str("#[allow(clippy::all)]\n");
    code.push_str("static ENCODED_PATHS: &[&[u8]] = &[\n");
    for (path, _) in &entries {
        let enc: Vec<u8> = path
            .bytes()
            .enumerate()
            .map(|(i, b)| b ^ path_key[i % path_key.len()])
            .collect();
        code.push_str(&format!("    &[\n{}\n    ],\n", rust_array(&enc)));
    }
    code.push_str("];\n");

    // embedded_resource_paths(): decode all paths once, cache in a OnceLock.
    code.push_str("#[allow(clippy::all)]\n");
    code.push_str("pub fn embedded_resource_paths() -> &'static [&'static str] {\n");
    code.push_str("    use std::sync::OnceLock;\n");
    code.push_str("    static PATHS: OnceLock<Vec<&'static str>> = OnceLock::new();\n");
    code.push_str("    PATHS.get_or_init(|| {\n");
    code.push_str(
        "        let mut v: Vec<&'static str> = Vec::with_capacity(ENCODED_PATHS.len());\n",
    );
    code.push_str("        for e in ENCODED_PATHS {\n");
    code.push_str("            let s = decode_path(e);\n");
    code.push_str("            v.push(Box::leak(s.into_boxed_str()));\n");
    code.push_str("        }\n");
    code.push_str("        v\n");
    code.push_str("    }).as_slice()\n");
    code.push_str("}\n");

    // embedded_resource(): match a decoded path against the encoded constants.
    code.push_str("#[allow(clippy::all)]\n");
    code.push_str("pub fn embedded_resource(path: &str) -> Option<&'static [u8]> {\n");
    code.push_str("    let enc: Vec<u8> = path.bytes().enumerate()\n");
    code.push_str("        .map(|(i, b)| b ^ PATH_XOR_KEY[i % PATH_XOR_KEY.len()])\n");
    code.push_str("        .collect();\n");
    code.push_str("    match enc.as_slice() {\n");

    // Write each encrypted blob to OUT_DIR and emit a match arm keyed on the
    // encoded path bytes.
    for (i, (path, data)) in entries.iter().enumerate() {
        let blob_name = format!("res_{}.bin", i);
        let blob_path = out_dir.join(&blob_name);
        fs::write(&blob_path, data).unwrap();
        let enc: Vec<u8> = path
            .bytes()
            .enumerate()
            .map(|(j, b)| b ^ path_key[j % path_key.len()])
            .collect();
        code.push_str(&format!(
            "        [\n{}\n        ] => Some(include_bytes!(\"{}\")),\n",
            rust_array(&enc),
            blob_path.to_string_lossy().replace('\\', "/")
        ));
    }
    code.push_str("        _ => None,\n");
    code.push_str("    }\n");
    code.push_str("}\n");

    let gen_path = out_dir.join("embedded_resources.rs");
    fs::write(&gen_path, code).unwrap();
    println!("cargo:rerun-if-changed=../../resources");
}
