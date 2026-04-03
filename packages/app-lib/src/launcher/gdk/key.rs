use hex;
use std::arch::x86_64::*;
use tracing::warn;
use uuid::Uuid;

#[derive(Clone, Copy)]
pub struct CikKey {
    pub guid: Uuid,
    pub t_key: [u8; 16],
    pub d_key: [u8; 16],
}

impl CikKey {
    pub const MAX_SIZE: usize = 0x30;

    pub fn new(cik: &[u8]) -> Result<Self, String> {
        if cik.len() < Self::MAX_SIZE {
            return Err(format!("CIK key is too short: {}", cik.len()));
        }

        let guid_bytes: [u8; 16] = cik[..0x10].try_into().unwrap();
        // Microsoft GUID bytes in these files are stored in little-endian form.
        let guid = Uuid::from_bytes_le(guid_bytes);
        let t_key = cik[0x10..0x20].try_into().unwrap();
        let d_key = cik[0x20..0x30].try_into().unwrap();

        Ok(Self { guid, t_key, d_key })
    }

    pub fn find_and_create(data: &[u8], expected_guid_str: &str) -> Result<Self, String> {
        let expected_guid =
            Uuid::parse_str(expected_guid_str).map_err(|e| format!("Invalid GUID: {}", e))?;
        let expected_bytes = expected_guid.to_bytes_le();

        if let Ok(key) = Self::new(data)
            && key.guid == expected_guid
        {
            return Ok(key);
        }

        if let Some(start_idx) = data.windows(16).position(|w| w == expected_bytes) {
            warn!("Auto-corrected CIK offset: {}", start_idx);
            if start_idx + Self::MAX_SIZE <= data.len() {
                return Self::new(&data[start_idx..start_idx + Self::MAX_SIZE]);
            }
        }

        // Fallback: try to parse a hex-encoded representation.
        if let Ok(text) = std::str::from_utf8(data) {
            let clean_hex: String = text.chars().filter(|c| c.is_ascii_hexdigit()).collect();
            if clean_hex.len() >= 96
                && let Ok(key) = Self::from_hex_string(&clean_hex[..96])
                && key.guid == expected_guid
            {
                return Ok(key);
            }
        }

        Err("Failed to find a matching CIK key".to_string())
    }

    pub fn from_hex_string(hex_string: &str) -> Result<Self, String> {
        let cik = hex::decode(hex_string).map_err(|e| e.to_string())?;
        Self::new(&cik)
    }
}

#[derive(Clone, Copy)]
pub struct KeySinagl {
    pub keys: [__m128i; 11],
}

// Helper macro for a single AES key schedule expansion round.
macro_rules! expand_round {
    ($rkeys:expr, $pos:expr, $rcon:expr) => {
        let temp = _mm_aeskeygenassist_si128($rkeys[$pos - 1], $rcon);
        $rkeys[$pos] = key_expansion($rkeys[$pos - 1], temp);
    };
}

impl KeySinagl {
    #[target_feature(enable = "sse2", enable = "aes")]
    pub unsafe fn new(key_bytes: &[u8], is_decryption: bool) -> Self {
        let mut keys = [_mm_setzero_si128(); 11];

        // Load initial key (unaligned load).
        keys[0] = _mm_loadu_si128(key_bytes.as_ptr() as *const __m128i);

        // AES-128 key schedule with standard RCON values.
        expand_round!(keys, 1, 0x01);
        expand_round!(keys, 2, 0x02);
        expand_round!(keys, 3, 0x04);
        expand_round!(keys, 4, 0x08);
        expand_round!(keys, 5, 0x10);
        expand_round!(keys, 6, 0x20);
        expand_round!(keys, 7, 0x40);
        expand_round!(keys, 8, 0x80);
        expand_round!(keys, 9, 0x1b);
        expand_round!(keys, 10, 0x36);

        if is_decryption {
            // For decryption, apply InverseMixColumns to middle round keys.
            for key in keys.iter_mut().take(10).skip(1) {
                *key = _mm_aesimc_si128(*key);
            }
        }

        Self { keys }
    }

    #[inline]
    #[target_feature(enable = "sse2", enable = "aes")]
    pub unsafe fn decrypt_block_unrolled(&self, input: __m128i) -> __m128i {
        let mut state = _mm_xor_si128(input, self.keys[10]);
        state = _mm_aesdec_si128(state, self.keys[9]);
        state = _mm_aesdec_si128(state, self.keys[8]);
        state = _mm_aesdec_si128(state, self.keys[7]);
        state = _mm_aesdec_si128(state, self.keys[6]);
        state = _mm_aesdec_si128(state, self.keys[5]);
        state = _mm_aesdec_si128(state, self.keys[4]);
        state = _mm_aesdec_si128(state, self.keys[3]);
        state = _mm_aesdec_si128(state, self.keys[2]);
        state = _mm_aesdec_si128(state, self.keys[1]);
        _mm_aesdeclast_si128(state, self.keys[0])
    }

    #[inline]
    #[target_feature(enable = "sse2", enable = "aes")]
    pub unsafe fn encrypt_unrolled(&self, input: __m128i) -> __m128i {
        let mut state = _mm_xor_si128(input, self.keys[0]);
        state = _mm_aesenc_si128(state, self.keys[1]);
        state = _mm_aesenc_si128(state, self.keys[2]);
        state = _mm_aesenc_si128(state, self.keys[3]);
        state = _mm_aesenc_si128(state, self.keys[4]);
        state = _mm_aesenc_si128(state, self.keys[5]);
        state = _mm_aesenc_si128(state, self.keys[6]);
        state = _mm_aesenc_si128(state, self.keys[7]);
        state = _mm_aesenc_si128(state, self.keys[8]);
        state = _mm_aesenc_si128(state, self.keys[9]);
        _mm_aesenclast_si128(state, self.keys[10])
    }
}

// Helper function matching C# key expansion logic.
#[inline]
#[target_feature(enable = "sse2")]
unsafe fn key_expansion(s: __m128i, t: __m128i) -> __m128i {
    let t = _mm_shuffle_epi32(t, 0xFF);
    let mut s = s;
    let s_shifted_4 = _mm_slli_si128(s, 4);
    s = _mm_xor_si128(s, s_shifted_4);
    let s_shifted_8 = _mm_slli_si128(s, 8);
    s = _mm_xor_si128(s, s_shifted_8);
    _mm_xor_si128(s, t)
}
