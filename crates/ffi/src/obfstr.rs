//! Compile-time string obfuscation.
//!
//! Sensitive string literals (magic headers, resource paths, error messages)
//! are XOR-encoded at compile time so they do not appear as plaintext in the
//! binary. `strings`/static scans will not find them. The decoded value is
//! produced at runtime on first use.
//!
//! This is an anti-forensics measure that raises the cost of casual extraction;
//! it is not a security boundary.
//!
//! IMPORTANT: the decode MUST happen at runtime (not in a `const`), otherwise
//! the optimizer constant-folds the XOR and emits the plaintext directly into
//! the binary, defeating the obfuscation. The encoded bytes are stored in a
//! `static` and decoded into a runtime buffer.

/// XOR-encode a string literal at compile time and decode it at runtime.
///
/// Usage: `obfstr!("manifest.toml")` yields a `&'static str`.
///
/// The decoded value is cached in a `OnceLock` so the plaintext is produced at
/// runtime (not constant-folded into the binary) and only once.
macro_rules! obfstr {
    ($s:expr) => {{
        const KEY: u8 = 0x5c;
        const LEN: usize = $s.len();
        // Encoded bytes stored as a static — this is what appears in the binary.
        static ENC: [u8; LEN] = {
            let bytes = $s.as_bytes();
            let mut out = [0u8; LEN];
            let mut i = 0;
            while i < LEN {
                out[i] = bytes[i] ^ KEY;
                i += 1;
            }
            out
        };
        static DECODED: std::sync::OnceLock<&'static str> = std::sync::OnceLock::new();
        *DECODED.get_or_init(|| {
            let mut buf = [0u8; LEN];
            let mut i = 0;
            while i < LEN {
                buf[i] = ENC[i] ^ KEY;
                i += 1;
            }
            // SAFETY: buf is a valid UTF-8 copy of the original literal.
            let s = unsafe { std::str::from_utf8_unchecked(&buf) };
            // Leak a copy so it has 'static lifetime.
            Box::leak(s.to_owned().into_boxed_str())
        })
    }};
}

pub(crate) use obfstr;

/// XOR-encode a byte-string literal at compile time and decode it at runtime,
/// yielding an owned `[u8; N]`. Useful for magic headers compared against byte
/// slices.
macro_rules! obfbytes {
    ($s:expr) => {{
        const KEY: u8 = 0x5c;
        const LEN: usize = $s.len();
        static ENC: [u8; LEN] = {
            let bytes = $s;
            let mut out = [0u8; LEN];
            let mut i = 0;
            while i < LEN {
                out[i] = bytes[i] ^ KEY;
                i += 1;
            }
            out
        };
        let mut buf = [0u8; LEN];
        let mut i = 0;
        while i < LEN {
            buf[i] = ENC[i] ^ KEY;
            i += 1;
        }
        buf
    }};
}

pub(crate) use obfbytes;
