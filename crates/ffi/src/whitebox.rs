//! Whitebox key derivation via a Feistel network.
//!
//! The master resource key is never stored as a contiguous byte string in the
//! binary. `build.rs` computes `embedded = feistel(key)` and embeds `embedded`
//! (masked and interleaved with garbage). At runtime, `derive_master_key()`
//! computes `key = feistel_inverse(embedded)`.
//!
//! A Feistel network is *always* invertible regardless of the round function,
//! so the round function can be a strong non-linear S-box mix without needing
//! to invert it. Recovering the key statically requires an attacker to:
//!   1. Locate the embedded seed (interleaved + XOR-masked, not contiguous).
//!   2. Locate the S-box and permutation constants (also interleaved).
//!   3. Reverse the Feistel rounds to recover the key.
//!
//! This is substantially harder than the previous single-XOR-mask scheme and
//! is intended to push a casual cracker past the 24h mark. It is still not a
//! hard security boundary — a determined reverse engineer can always recover a
//! key that must exist in the binary.

struct Tables {
    sbox: [u8; 256],
    perm: [u8; 32],
    masked_seed: [u8; 32],
    seed_mask: [u8; 32],
}

static TABLES: std::sync::OnceLock<Tables> = std::sync::OnceLock::new();

/// Number of Feistel rounds.
const ROUNDS: usize = 10;

/// Initialize the SPN tables from the build-time constants. Called once.
///
/// The constants are provided by `build.rs` as several scattered fragments,
/// each included via its own `include_bytes!`. Using raw bytes (rather than a
/// Rust const array) prevents the optimizer from constant-folding the
/// de-interleaving and emitting the raw key/mask as a contiguous constant in
/// the binary.
///
/// Each fragment is stored interleaved (real byte at even index, garbage at
/// odd index) with a per-fragment XOR mask and per-fragment garbage affine
/// params. We read with volatile loads so the optimizer cannot constant-fold
/// the de-interleaving and emit the raw key/mask as a contiguous constant.
fn tables() -> &'static Tables {
    TABLES.get_or_init(|| {
        let mut logical = [0u8; 256 + 32 + 32 + 32];
        for frag in SPN_FRAGMENTS {
            let src = frag.data;
            for j in 0..frag.len {
                // SAFETY: j is bounded by the generated fragment length.
                let real = unsafe { std::ptr::read_volatile(src.as_ptr().add(j * 2)) };
                let garbage = unsafe { std::ptr::read_volatile(src.as_ptr().add(j * 2 + 1)) };
                assert_eq!(garbage, real.wrapping_mul(frag.mult).wrapping_add(frag.add));
                logical[frag.offset + j] = real ^ frag.mask[j];
            }
        }
        let mut tables = Tables {
            sbox: [0; 256],
            perm: [0; 32],
            masked_seed: [0; 32],
            seed_mask: [0; 32],
        };
        tables.sbox.copy_from_slice(&logical[0..256]);
        tables.perm.copy_from_slice(&logical[256..288]);
        tables.masked_seed.copy_from_slice(&logical[288..320]);
        tables.seed_mask.copy_from_slice(&logical[320..352]);
        tables
    })
}

/// The Feistel round function: a non-linear, non-invertible mix of the right
/// half with a round key derived from the round index.
fn round_function(right: &[u8; 16], round: usize) -> [u8; 16] {
    let mut out = [0u8; 16];
    let tables = tables();
    for i in 0..16 {
        let x = right[i];
        // S-box substitution, then mix with a round-dependent constant.
        let s = tables.sbox[x as usize];
        let mixed = s
            .wrapping_add((round as u8).wrapping_mul(0x2f))
            .wrapping_add(0x53)
            .wrapping_add(right[(i + 5) % 16]);
        out[i] = tables.sbox[mixed as usize];
    }
    out
}

/// Apply the byte permutation to a 32-byte block.
#[cfg(test)]
fn permute(block: &mut [u8; 32]) {
    let tables = tables();
    let mut tmp = [0u8; 32];
    for i in 0..32 {
        tmp[i] = block[tables.perm[i] as usize];
    }
    *block = tmp;
}

/// Inverse of `permute`.
fn unpermute(block: &mut [u8; 32]) {
    let tables = tables();
    let mut tmp = [0u8; 32];
    for i in 0..32 {
        tmp[tables.perm[i] as usize] = block[i];
    }
    *block = tmp;
}

/// Forward Feistel: `embedded = feistel(key)`. Used by `build.rs` (mirrored in
/// Rust here for the roundtrip test) and conceptually by the runtime inverse.
#[cfg(test)]
fn feistel_forward(block: &mut [u8; 32]) {
    for round in 0..ROUNDS {
        // Split into left/right halves.
        let mut left = [0u8; 16];
        let mut right = [0u8; 16];
        left.copy_from_slice(&block[0..16]);
        right.copy_from_slice(&block[16..32]);
        // Feistel round: L, R = R, L XOR F(R).
        let f = round_function(&right, round);
        let new_left = right;
        let mut new_right = [0u8; 16];
        for i in 0..16 {
            new_right[i] = left[i] ^ f[i];
        }
        block[0..16].copy_from_slice(&new_left);
        block[16..32].copy_from_slice(&new_right);
        // Diffuse across the whole block between rounds.
        permute(block);
    }
}

/// Inverse Feistel: `key = feistel_inverse(embedded)`.
fn feistel_inverse(block: &mut [u8; 32]) {
    for round in (0..ROUNDS).rev() {
        // Undo the permutation applied after this round in the forward pass.
        unpermute(block);
        let mut left = [0u8; 16];
        let mut right = [0u8; 16];
        left.copy_from_slice(&block[0..16]);
        right.copy_from_slice(&block[16..32]);
        // In a Feistel round, new_left = old_right, new_right = old_left XOR F(old_right).
        // So old_right = new_left, old_left = new_right XOR F(new_left).
        let f = round_function(&left, round);
        let old_right = left;
        let mut old_left = [0u8; 16];
        for i in 0..16 {
            old_left[i] = right[i] ^ f[i];
        }
        block[0..16].copy_from_slice(&old_left);
        block[16..32].copy_from_slice(&old_right);
    }
}

/// Reconstruct the 32-byte master resource key.
pub fn derive_master_key() -> [u8; 32] {
    let tables = tables();
    let mut seed = [0u8; 32];
    for (index, byte) in seed.iter_mut().enumerate() {
        *byte = tables.masked_seed[index] ^ tables.seed_mask[index];
    }

    // Invert the Feistel to recover the key.
    feistel_inverse(&mut seed);
    seed
}

/// Compute the embedded seed from a key (forward Feistel). Exposed for the
/// roundtrip test and for `build.rs` to mirror.
#[cfg(test)]
pub fn embed_key(key: &[u8; 32]) -> [u8; 32] {
    let _ = tables();
    let mut block = *key;
    feistel_forward(&mut block);
    block
}

// The SPN tables are generated by build.rs as scattered opaque fragments.
include!(concat!(env!("OUT_DIR"), "/spn_fragments.rs"));

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn feistel_roundtrip() {
        // A known key.
        let key = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb, 0xcc, 0xdd,
            0xee, 0xff, 0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xaa, 0xbb,
            0xcc, 0xdd, 0xee, 0xff,
        ];
        let embedded = embed_key(&key);
        // The embedded seed must differ from the key (obfuscation is real).
        assert_ne!(embedded, key, "embedded seed should differ from the key");
        // Inverting the embedded seed must recover the key.
        let mut block = embedded;
        feistel_inverse(&mut block);
        assert_eq!(block, key, "feistel_inverse(feistel(key)) must equal key");
    }

    #[test]
    fn derive_master_key_is_stable() {
        // derive_master_key() must be deterministic within a build.
        let a = derive_master_key();
        let b = derive_master_key();
        assert_eq!(a, b, "master key derivation must be deterministic");
    }
}
