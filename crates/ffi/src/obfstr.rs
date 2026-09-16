macro_rules! obfstr {
    ($s:expr) => {{
        const KEY: u8 = 0x5c;
        const LEN: usize = $s.len();
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
