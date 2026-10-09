// hashsum core — pure-Rust digest implementations for the algorithms listed in
// docs/man/cksum.1.gz (DIGEST section): sysv, bsd, crc, crc32b, md5, sha1, sha2,
// sha3, blake2b, sm3, sha224, sha256, sha384, sha512, blake3, shake128, shake256.
//
// No external crates (the project has no dependencies).
//
// Assumptions (docs/assumptions.md):
//   - crc32b is the Castagnoli CRC-32C polynomial (the man page lists the name only);
//   - shake128/shake256 default output length is 32 bytes when no length is given;
//   - blake2b default output length is 512 bits (verified against the reference); blake3 default is 256 bits.
//   - shake128/shake256 accept any bit length (rounded up to whole bytes); blake2b/blake3 require a multiple of 8.

pub struct Algo {
    pub name: &'static str,
    pub bits: usize,
    pub display: &'static str,
}

pub const CRC: Algo = Algo { name: "crc", bits: 32, display: "CRC" };
pub const CRC32B: Algo = Algo { name: "crc32b", bits: 32, display: "CRC32b" };
pub const SYSV: Algo = Algo { name: "sysv", bits: 16, display: "SysV" };
pub const BSD: Algo = Algo { name: "bsd", bits: 16, display: "BSD" };
pub const MD5: Algo = Algo { name: "md5", bits: 128, display: "MD5" };
pub const SHA1: Algo = Algo { name: "sha1", bits: 160, display: "SHA1" };
pub const SHA2: Algo = Algo { name: "sha2", bits: 256, display: "SHA2" };
pub const SHA3: Algo = Algo { name: "sha3", bits: 256, display: "SHA3" };
pub const BLAKE2B: Algo = Algo { name: "blake2b", bits: 512, display: "BLAKE2b" };
pub const SM3: Algo = Algo { name: "sm3", bits: 256, display: "SM3" };
pub const SHA224: Algo = Algo { name: "sha224", bits: 224, display: "SHA224" };
pub const SHA256: Algo = Algo { name: "sha256", bits: 256, display: "SHA256" };
pub const SHA384: Algo = Algo { name: "sha384", bits: 384, display: "SHA384" };
pub const SHA512: Algo = Algo { name: "sha512", bits: 512, display: "SHA512" };
pub const BLAKE3: Algo = Algo { name: "blake3", bits: 256, display: "BLAKE3" };
pub const SHAKE128: Algo = Algo { name: "shake128", bits: 256, display: "SHAKE128" };
pub const SHAKE256: Algo = Algo { name: "shake256", bits: 512, display: "SHAKE256" };

// sha3-*sum applets：同一演算法、固定輸出長度（man page 的 -l 值）。
pub const SHA3_224: Algo = Algo { name: "sha3", bits: 224, display: "SHA3" };
pub const SHA3_256: Algo = Algo { name: "sha3", bits: 256, display: "SHA3" };
pub const SHA3_384: Algo = Algo { name: "sha3", bits: 384, display: "SHA3" };
pub const SHA3_512: Algo = Algo { name: "sha3", bits: 512, display: "SHA3" };

pub fn digest(algo: &Algo, data: &[u8], out_bits: usize) -> Vec<u8> {
    let bits = if out_bits == 0 { algo.bits } else { out_bits };
    match algo.name {
        "crc" => u32le(crc32_msb(data, &crc_table_msb(0x04c11db7))),
        "crc32b" => u32le(crc32_ieee(data)),
        "sysv" => u32le(sysv(data)),
        "bsd" => u32le(bsd(data)),
        "md5" => md5(data),
        "sha1" => sha1(data),
        "sm3" => sm3(data),
        "blake2b" => blake2b(data, bits / 8),
        "blake3" => blake3(data, bits / 8),
        // shake accepts any bit length; the reference rounds up to whole bytes
        "shake128" => keccak(data, (bits + 7) / 8, 32, 0x1f),
        "shake256" => keccak(data, (bits + 7) / 8, 64, 0x1f),
        "sha3" | "sha3-224" | "sha3-256" | "sha3-384" | "sha3-512" => keccak(data, bits / 8, 2 * (bits / 8), 0x06),
        "sha2" | "sha224" | "sha256" => sha256(data, bits),
        "sha384" | "sha512" => sha512(data, bits),
        _ => Vec::new(),
    }
}

fn u32le(v: u32) -> Vec<u8> {
    vec![(v & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, ((v >> 16) & 0xFF) as u8, ((v >> 24) & 0xFF) as u8]
}

// ---------------------------------------------------------------- CRC

pub const fn crc_table(poly: u32) -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut c = i as u32;
        let mut k = 0;
        while k < 8 {
            c = if c & 1 != 0 { (c >> 1) ^ poly } else { c >> 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

pub fn crc32(data: &[u8], table: &[u32; 256]) -> u32 {
    let mut c = 0u32;
    for &b in data {
        c = table[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c
}

// 非反射（MSB-first）CRC-32 表與演算法：本專案對 `crc` 的定義（docs/assumptions.md）。
pub const fn crc_table_msb(poly: u32) -> [u32; 256] {
    let mut t = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        // MSB-first table: the entry is the 8-bit processing of `i << 24`.
        let mut c = (i as u32) << 24;
        let mut k = 0;
        while k < 8 {
            c = if c & 0x80000000 != 0 { (c << 1) ^ poly } else { c << 1 };
            k += 1;
        }
        t[i] = c;
        i += 1;
    }
    t
}

pub fn crc32_msb(data: &[u8], table: &[u32; 256]) -> u32 {
    let mut c = 0u32;
    for &b in data {
        c = table[(((c >> 24) ^ b as u32) & 0xFF) as usize] ^ (c << 8);
    }
    c
}

// crc32b：uutils 0.10.0 的實際行為 = 標準 CRC-32 (IEEE/zlib)，
// 反射表 poly 0xedb88320、init 0xFFFFFFFF、最後 xor 0xFFFFFFFF。
// （man page 只列名字；名字上像 Castagnoli，但參考實作 4 個輸入全部吻合 zlib。）
pub fn crc32_ieee(data: &[u8]) -> u32 {
    let t = crc_table(0xedb88320);
    let mut c = 0xFFFFFFFFu32;
    for &b in data {
        c = t[((c ^ b as u32) & 0xFF) as usize] ^ (c >> 8);
    }
    c ^ 0xFFFFFFFF
}

// ---------------------------------------------------------------- sysv / bsd sum

pub fn sysv(data: &[u8]) -> u32 {
    let mut s = 0u32;
    for &b in data {
        s = s.wrapping_add(b as u32);
    }
    s & 0xFFFF
}

pub fn bsd(data: &[u8]) -> u32 {
    let mut s = 0u32;
    for &b in data {
        s = (s >> 1) + if s & 1 == 1 { 0x8000 } else { 0 };
        s = (s + b as u32) & 0xFFFF;
    }
    s & 0xFFFF
}

// ---------------------------------------------------------------- MD5

const MD5_K: [u32; 64] = [
    0xd76aa478, 0xe8c7b756, 0x242070db, 0xc1bdceee, 0xf57c0faf, 0x4787c62a, 0xa8304613, 0xfd469501,
    0x698098d8, 0x8b44f7af, 0xffff5bb1, 0x895cd7be, 0x6b901122, 0xfd987193, 0xa679438e, 0x49b40821,
    0xf61e2562, 0xc040b340, 0x265e5a51, 0xe9b6c7aa, 0xd62f105d, 0x02441453, 0xd8a1e681, 0xe7d3fbc8,
    0x21e1cde6, 0xc33707d6, 0xf4d50d87, 0x455a14ed, 0xa9e3e905, 0xfcefa3f8, 0x676f02d9, 0x8d2a4c8a,
    0xfffa3942, 0x8771f681, 0x6d9d6122, 0xfde5380c, 0xa4beea44, 0x4bdecfa9, 0xf6bb4b60, 0xbebfbc70,
    0x289b7ec6, 0xeaa127fa, 0xd4ef3085, 0x04881d05, 0xd9d4d039, 0xe6db99e5, 0x1fa27cf8, 0xc4ac5665,
    0xf4292244, 0x432aff97, 0xab9423a7, 0xfc93a039, 0x655b59c3, 0x8f0ccc92, 0xffeff47d, 0x85845dd1,
    0x6fa87e4f, 0xfe2ce6e0, 0xa3014314, 0x4e0811a1, 0xf7537e82, 0xbd3af235, 0x2ad7d2bb, 0xeb86d391,
];



// 每個 group 內，accumulator 的順序是 a, d, c, b；S 依 position 取用。
const MD5_S4: [[u32; 4]; 4] = [[7, 12, 17, 22], [5, 9, 14, 20], [4, 11, 16, 23], [6, 10, 15, 21]];

fn bool3(r: usize, x: u32, y: u32, z: u32) -> u32 {
    match r {
        0 => (x & y) | (!x & z),
        1 => (x & z) | (y & !z),
        2 => x ^ y ^ z,
        _ => y ^ (x | !z),
    }
}

pub fn md5(data: &[u8]) -> Vec<u8> {
    let mut h = [0x67452301u32, 0xefcdab89, 0x98badcfe, 0x10325476];
    for block in blocks(data, 64, true) {
        let mut w = [0u32; 16];
        for i in 0..16 {
            let j = i * 4;
            w[i] = (block[j] as u32)
                | ((block[j + 1] as u32) << 8)
                | ((block[j + 2] as u32) << 16)
                | ((block[j + 3] as u32) << 24);
        }
        let mut v = h;
        for i in 0..64 {
            let r = i / 16;
            let j = i % 4;
            // 每個 group 內的呼叫順序是 FF(a,b,c,d), FF(d,a,b,c), FF(c,d,a,b), FF(b,c,d,a)
            let idx = [0usize, 3, 2, 1][j];
            let p = [v[1], v[0], v[3], v[2]][j];
            let q = [v[2], v[1], v[0], v[3]][j];
            let z = [v[3], v[2], v[1], v[0]][j];
            let g = [i, (5 * i + 1) % 16, (3 * i + 5) % 16, (7 * i) % 16][r];
            let f = bool3(r, p, q, z);
            v[idx] = p.wrapping_add(
                v[idx]
                    .wrapping_add(f)
                    .wrapping_add(MD5_K[i])
                    .wrapping_add(w[g])
                    .rotate_left(MD5_S4[r][j]),
            );
        }
        for i in 0..4 {
            h[i] = h[i].wrapping_add(v[i]);
        }
    }
    let mut out = Vec::new();
    for v in h {
        out.extend(u32le(v));
    }
    out
}

// ---------------------------------------------------------------- SHA-1

const SHA1_K: [u32; 4] = [0x5a827999, 0x6ed9eba1, 0x8f1bbcdc, 0xca62c1d6];

pub fn sha1(data: &[u8]) -> Vec<u8> {
    let mut h = [0x67452301u32, 0xefcdab89, 0x98badcfe, 0x10325476, 0xc3d2e1f0];
    for block in blocks(data, 64, false) {
        let mut w = [0u32; 80];
        for i in 0..16 {
            let j = i * 4;
            w[i] = ((block[j] as u32) << 24)
                | ((block[j + 1] as u32) << 16)
                | ((block[j + 2] as u32) << 8)
                | block[j + 3] as u32;
        }
        for i in 16..80 {
            w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        for i in 0..80 {
            let (f, k) = match i / 20 {
                0 => ((b & c) | (!b & d), SHA1_K[0]),
                1 => (b ^ c ^ d, SHA1_K[1]),
                2 => ((b & c) | (b & d) | (c & d), SHA1_K[2]),
                _ => (b ^ c ^ d, SHA1_K[3]),
            };
            let tmp = a
                .rotate_left(5)
                .wrapping_add(f)
                .wrapping_add(e)
                .wrapping_add(k)
                .wrapping_add(w[i]);
            e = d;
            d = c;
            c = b.rotate_left(30);
            b = a;
            a = tmp;
        }
        h[0] = h[0].wrapping_add(a);
        h[1] = h[1].wrapping_add(b);
        h[2] = h[2].wrapping_add(c);
        h[3] = h[3].wrapping_add(d);
        h[4] = h[4].wrapping_add(e);
    }
    let mut out = Vec::new();
    for v in h {
        out.extend(u32be(v));
    }
    out
}

fn u32be(v: u32) -> Vec<u8> {
    vec![(v >> 24) as u8, ((v >> 16) & 0xFF) as u8, ((v >> 8) & 0xFF) as u8, (v & 0xFF) as u8]
}

// ---------------------------------------------------------------- SHA-256 / SHA-224

const SHA256_K: [u32; 64] = [
    0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
    0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
    0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
    0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
    0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
    0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
    0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
    0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
];

pub fn sha256(data: &[u8], out_bits: usize) -> Vec<u8> {
    let mut h: [u32; 8] = if out_bits == 224 {
        [0xc1059ed8, 0x367cd507, 0x3070dd17, 0xf70e5939, 0xffc00b31, 0x68581511, 0x64f98fa7, 0xbefa4fa4]
    } else {
        [0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19]
    };
    for block in blocks(data, 64, false) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            let j = i * 4;
            w[i] = ((block[j] as u32) << 24)
                | ((block[j + 1] as u32) << 16)
                | ((block[j + 2] as u32) << 8)
                | block[j + 3] as u32;
        }
        for i in 16..64 {
            let s0 = w[i - 15].rotate_right(7) ^ w[i - 15].rotate_right(18) ^ (w[i - 15] >> 3);
            let s1 = w[i - 2].rotate_right(17) ^ w[i - 2].rotate_right(19) ^ (w[i - 2] >> 10);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];
        for i in 0..64 {
            let s1 = e.rotate_right(6) ^ e.rotate_right(11) ^ e.rotate_right(25);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(SHA256_K[i]).wrapping_add(w[i]);
            let s0 = a.rotate_right(2) ^ a.rotate_right(13) ^ a.rotate_right(22);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (i, v) in [a, b, c, d, e, f, g, hh].iter().enumerate() {
            h[i] = h[i].wrapping_add(*v);
        }
    }
    let mut out = Vec::new();
    for v in h {
        out.extend(u32be(v));
    }
    out.truncate(out_bits / 8);
    out
}

// ---------------------------------------------------------------- SHA-512 / SHA-384

const SHA512_K: [u64; 80] = [
    0x428a2f98d728ae22, 0x7137449123ef65cd, 0xb5c0fbcfec4d3b2f, 0xe9b5dba58189dbbc, 0x3956c25bf348b538,
    0x59f111f1b605d019, 0x923f82a4af194f9b, 0xab1c5ed5da6d8118, 0xd807aa98a3030242, 0x12835b0145706fbe,
    0x243185be4ee4b28c, 0x550c7dc3d5ffb4e2, 0x72be5d74f27b896f, 0x80deb1fe3b1696b1, 0x9bdc06a725c71235,
    0xc19bf174cf692694, 0xe49b69c19ef14ad2, 0xefbe4786384f25e3, 0x0fc19dc68b8cd5b5, 0x240ca1cc77ac9c65,
    0x2de92c6f592b0275, 0x4a7484aa6ea6e483, 0x5cb0a9dcbd41fbd4, 0x76f988da831153b5, 0x983e5152ee66dfab,
    0xa831c66d2db43210, 0xb00327c898fb213f, 0xbf597fc7beef0ee4, 0xc6e00bf33da88fc2, 0xd5a79147930aa725,
    0x06ca6351e003826f, 0x142929670a0e6e70, 0x27b70a8546d22ffc, 0x2e1b21385c26c926, 0x4d2c6dfc5ac42aed,
    0x53380d139d95b3df, 0x650a73548baf63de, 0x766a0abb3c77b2a8, 0x81c2c92e47edaee6, 0x92722c851482353b,
    0xa2bfe8a14cf10364, 0xa81a664bbc423001, 0xc24b8b70d0f89791, 0xc76c51a30654be30, 0xd192e819d6ef5218,
    0xd69906245565a910, 0xf40e35855771202a, 0x106aa07032bbd1b8, 0x19a4c116b8d2d0c8, 0x1e376c085141ab53,
    0x2748774cdf8eeb99, 0x34b0bcb5e19b48a8, 0x391c0cb3c5c95a63, 0x4ed8aa4ae3418acb, 0x5b9cca4f7763e373,
    0x682e6ff3d6b2b8a3, 0x748f82ee5defb2fc, 0x78a5636f43172f60, 0x84c87814a1f0ab72, 0x8cc702081a6439ec,
    0x90befffa23631e28, 0xa4506cebde82bde9, 0xbef9a3f7b2c67915, 0xc67178f2e372532b, 0xca273eceea26619c,
    0xd186b8c721c0c207, 0xeada7dd6cde0eb1e, 0xf57d4f7fee6ed178, 0x06f067aa72176fba, 0x0a637dc5a2c898a6,
    0x113f9804bef90dae, 0x1b710b35131c471b, 0x28db77f523047d84, 0x32caab7b40c72493, 0x3c9ebe0a15c9bebc,
    0x431d67c49c100d4c, 0x4cc5d4becb3e42b6, 0x597f299cfc657e2a, 0x5fcb6fab3ad6faec, 0x6c44198c4a475817,
];

pub fn sha512(data: &[u8], out_bits: usize) -> Vec<u8> {
    let mut h: [u64; 8] = if out_bits == 384 {
        [
            0xcbbb9d5dc1059ed8, 0x629a292a367cd507, 0x9159015a3070dd17, 0x152fecd8f70e5939,
            0x67332667ffc00b31, 0x8eb44a8768581511, 0xdb0c2e0d64f98fa7, 0x47b5481dbefa4fa4,
        ]
    } else {
        [
            0x6a09e667f3bcc908, 0xbb67ae8584caa73b, 0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1,
            0x510e527fade682d1, 0x9b05688c2b3e6c1f, 0x1f83d9abfb41bd6b, 0x5be0cd19137e2179,
        ]
    };
    for block in blocks(data, 128, false) {
        let mut w = [0u64; 80];
        for i in 0..16 {
            let j = i * 8;
            let mut v = 0u64;
            for k in 0..8 {
                v = (v << 8) | block[j + k] as u64;
            }
            w[i] = v;
        }
        for i in 16..80 {
            let s0 = w[i - 15].rotate_right(1) ^ w[i - 15].rotate_right(8) ^ (w[i - 15] >> 7);
            let s1 = w[i - 2].rotate_right(19) ^ w[i - 2].rotate_right(61) ^ (w[i - 2] >> 6);
            w[i] = w[i - 16]
                .wrapping_add(s0)
                .wrapping_add(w[i - 7])
                .wrapping_add(s1);
        }
        let mut a = h[0];
        let mut b = h[1];
        let mut c = h[2];
        let mut d = h[3];
        let mut e = h[4];
        let mut f = h[5];
        let mut g = h[6];
        let mut hh = h[7];
        for i in 0..80 {
            let s1 = e.rotate_right(14) ^ e.rotate_right(18) ^ e.rotate_right(41);
            let ch = (e & f) ^ (!e & g);
            let t1 = hh.wrapping_add(s1).wrapping_add(ch).wrapping_add(SHA512_K[i]).wrapping_add(w[i]);
            // FIPS 180-4 §5.4.1.2: Σ0(x) = ROT^1? no — Σ0(x) = ROT^28(x) ⊕ ROT^34(x) ⊕ ROT^39(x)
            // (ROT^1 ⊕ ROT^8 ⊕ SHR^7 is the message-schedule σ0, not Σ0.)
            let s0 = a.rotate_right(28) ^ a.rotate_right(34) ^ a.rotate_right(39);
            let maj = (a & b) ^ (a & c) ^ (b & c);
            let t2 = s0.wrapping_add(maj);
            hh = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }
        for (i, v) in [a, b, c, d, e, f, g, hh].iter().enumerate() {
            h[i] = h[i].wrapping_add(*v);
        }
    }
    let mut out = Vec::new();
    for v in h {
        for k in (0..8).rev() {
            out.push(((v >> (k * 8)) & 0xFF) as u8);
        }
    }
    out.truncate(out_bits / 8);
    out
}

// ---------------------------------------------------------------- Keccak / SHA-3 / SHAKE

// Round constants and rotation offsets are generated inside keccak_f from the
// FIPS 202 rules (LFSR for iota, cycle walk for rho+pi) rather than hard-coded
// tables, so they cannot drift from the spec.

// Keccak-f[1600], following the XKCP CompactFIPS202 reference exactly:
// theta, rho+pi (cycle walk starting at (1,0)), chi, iota (LFSR round constants).
fn keccak_f(a: &mut [u64; 25]) {
    let mut r = 1u64;
    for _ in 0..24 {
        // theta
        let mut c = [0u64; 5];
        for x in 0..5 {
            for y in 0..5 {
                c[x] ^= a[x + 5 * y];
            }
        }
        for x in 0..5 {
            let d = c[(x + 4) % 5] ^ c[(x + 1) % 5].rotate_left(1);
            for y in 0..5 {
                a[x + 5 * y] ^= d;
            }
        }
        // rho + pi: walk the cycle starting at (1,0); the element at p_t is rotated
        // by (t+1)(t+2)/2 and moved to p_{t+1} = (y, (2x+3y) mod 5).
        let mut x = 1usize;
        let mut y = 0usize;
        let mut cur = a[x + 5 * y];
        for t in 0..24 {
            let nx = y;
            let ny = (2 * x + 3 * y) % 5;
            x = nx;
            y = ny;
            let nxt = a[x + 5 * y];
            a[x + 5 * y] = cur.rotate_left(((t + 1) * (t + 2) / 2) as u32);
            cur = nxt;
        }
        // chi (reads the pre-chi row, so copy it per row)
        for y in 0..5 {
            let t = [a[0 + 5 * y], a[1 + 5 * y], a[2 + 5 * y], a[3 + 5 * y], a[4 + 5 * y]];
            for x in 0..5 {
                a[x + 5 * y] = t[x] ^ (!t[(x + 1) % 5]) & t[(x + 2) % 5];
            }
        }
        // iota
        for j in 0..7 {
            r = ((r << 1) ^ ((r >> 7) * 0x71)) % 256;
            if r & 2 != 0 {
                a[0] ^= 1u64 << ((1u64 << j) - 1);
            }
        }
    }
}

// cap_bytes is the capacity (not the output length): for SHA-3 it equals the
// output length, for SHAKE128/256 it is fixed at 32/64 bytes.
pub fn keccak(data: &[u8], out_bytes: usize, cap_bytes: usize, pad_byte: u8) -> Vec<u8> {
    let rate = 200 - cap_bytes;
    let mut state = [0u64; 25];
    let mut buf = data.to_vec();
    buf.push(pad_byte);
    while buf.len() % rate != 0 {
        buf.push(0);
    }
    let last = buf.len() - 1;
    buf[last] |= 0x80;
    for chunk in buf.chunks(rate) {
        // only rate/8 lanes are absorbed (the rest of the 25-lane state stays 0)
        for i in 0..rate / 8 {
            let lane = &mut state[i];
            let mut v = 0u64;
            for k in 0..8 {
                v |= (chunk[i * 8 + k] as u64) << (8 * k);
            }
            *lane ^= v;
        }
        keccak_f(&mut state);
    }
    let mut out = Vec::new();
    for lane in state.iter() {
        for k in 0..8 {
            out.push(((lane >> (k * 8)) & 0xFF) as u8);
        }
    }
    out.truncate(out_bytes);
    out
}

// ---------------------------------------------------------------- BLAKE2b

const B2_IV: [u64; 8] = [
    0x6a09e667f3bcc908, 0xbb67ae8584caa73b, 0x3c6ef372fe94f82b, 0xa54ff53a5f1d36f1,
    0x510e527fade682d1, 0x9b05688c2b3e6c1f, 0x1f83d9abfb41bd6b, 0x5be0cd19137e2179,
];

pub fn blake2b(data: &[u8], out_bytes: usize) -> Vec<u8> {
    let out_bytes = out_bytes.min(64);
    let mut h = B2_IV;
    h[0] ^= 0x01010000 ^ out_bytes as u64; // key length 0
    let mut total = 0u64;
    let mut i = 0;
    while i + 128 <= data.len() {
        let block = &data[i..i + 128];
        total += 128;
        compress_b2(&mut h, block, total, false);
        i += 128;
    }
    let mut last = vec![0u8; 128];
    let n = data.len() - i;
    last[..n].copy_from_slice(&data[i..]);
    total += n as u64;
    compress_b2(&mut h, &last, total, true);
    let mut out = Vec::new();
    for v in h.iter().take((out_bytes + 7) / 8) {
        for k in 0..8 {
            out.push(((v >> (k * 8)) & 0xFF) as u8);
        }
    }
    out.truncate(out_bytes);
    out
}

// BLAKE2b sigma (official ref). NOTE: this is NOT the ChaCha20 sigma — rows 1,2,4,5,6,8,9,10,11 differ.
const B2_SIGMA: [[usize; 16]; 12] = [
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
    [11, 8, 12, 0, 5, 2, 15, 13, 10, 14, 3, 6, 7, 1, 9, 4],
    [7, 9, 3, 1, 13, 12, 11, 14, 2, 6, 5, 10, 4, 0, 15, 8],
    [9, 0, 5, 7, 2, 4, 10, 15, 14, 1, 11, 12, 6, 8, 3, 13],
    [2, 12, 6, 10, 0, 11, 8, 3, 4, 13, 7, 5, 15, 14, 1, 9],
    [12, 5, 1, 15, 14, 13, 4, 10, 0, 7, 6, 3, 9, 2, 8, 11],
    [13, 11, 7, 14, 12, 1, 3, 9, 5, 0, 15, 4, 8, 6, 2, 10],
    [6, 15, 14, 9, 11, 3, 0, 8, 12, 2, 13, 7, 1, 4, 10, 5],
    [10, 2, 8, 4, 7, 6, 1, 5, 15, 11, 9, 14, 3, 12, 13, 0],
    [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    [14, 10, 4, 8, 9, 15, 13, 6, 1, 12, 0, 2, 11, 7, 5, 3],
];

fn compress_b2(h: &mut [u64; 8], block: &[u8], bytes: u64, last: bool) {
    let mut m = [0u64; 16];
    for i in 0..16 {
        let mut v = 0u64;
        for k in 0..8 {
            v |= (block[i * 8 + k] as u64) << (8 * k);
        }
        m[i] = v;
    }
    let mut v = [0u64; 16];
    for i in 0..8 {
        v[i] = h[i];
    }
    for i in 0..8 {
        v[8 + i] = B2_IV[i];
    }
    v[12] ^= bytes;
    if last {
        v[14] ^= u64::MAX;
    }
    for round in 0..12 {
        let s = &B2_SIGMA[round];
        mix_b2(&mut v, 0, 4, 8, 12, m[s[0]], m[s[1]]);
        mix_b2(&mut v, 1, 5, 9, 13, m[s[2]], m[s[3]]);
        mix_b2(&mut v, 2, 6, 10, 14, m[s[4]], m[s[5]]);
        mix_b2(&mut v, 3, 7, 11, 15, m[s[6]], m[s[7]]);
        // diagonal step: (0,5,10,15) (1,6,11,12) (2,7,8,13) (3,4,9,14) — not (0,1,2,3)...(12,13,14,15)
        mix_b2(&mut v, 0, 5, 10, 15, m[s[8]], m[s[9]]);
        mix_b2(&mut v, 1, 6, 11, 12, m[s[10]], m[s[11]]);
        mix_b2(&mut v, 2, 7, 8, 13, m[s[12]], m[s[13]]);
        mix_b2(&mut v, 3, 4, 9, 14, m[s[14]], m[s[15]]);
    }
    for i in 0..8 {
        h[i] ^= v[i] ^ v[i + 8];
    }
}

fn mix_b2(v: &mut [u64; 16], a: usize, b: usize, c: usize, d: usize, x: u64, y: u64) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(x);
    v[d] = (v[d] ^ v[a]).rotate_right(32);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(24);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(y);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(63);
}

// ---------------------------------------------------------------- BLAKE3

const B3_IV: [u32; 8] = [
    0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a, 0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
];

const CHUNK_START: u32 = 1 << 0;
const CHUNK_END: u32 = 1 << 1;
const PARENT: u32 = 1 << 2;
const ROOT: u32 = 1 << 3;

// BLAKE3 follows the official reference implementation
// (https://github.com/BLAKE3-team/BLAKE3/blob/master/reference_impl/reference_impl.rs).
const B3_MSG_PERMUTATION: [usize; 16] = [2, 6, 3, 10, 7, 0, 4, 13, 1, 11, 12, 5, 9, 14, 15, 8];

fn b3_words(block: &[u8]) -> [u32; 16] {
    let mut m = [0u32; 16];
    for (i, w) in m.iter_mut().enumerate() {
        let j = i * 4;
        if j + 4 <= block.len() {
            *w = u32::from_le_bytes([block[j], block[j + 1], block[j + 2], block[j + 3]]);
        }
    }
    m
}

// The mixing function G (used for both the column and the diagonal step).
fn g_b3(v: &mut [u32; 16], a: usize, b: usize, c: usize, d: usize, mx: u32, my: u32) {
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(mx);
    v[d] = (v[d] ^ v[a]).rotate_right(16);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(12);
    v[a] = v[a].wrapping_add(v[b]).wrapping_add(my);
    v[d] = (v[d] ^ v[a]).rotate_right(8);
    v[c] = v[c].wrapping_add(v[d]);
    v[b] = (v[b] ^ v[c]).rotate_right(7);
}

fn round_b3(v: &mut [u32; 16], m: &[u32; 16]) {
    g_b3(v, 0, 4, 8, 12, m[0], m[1]);
    g_b3(v, 1, 5, 9, 13, m[2], m[3]);
    g_b3(v, 2, 6, 10, 14, m[4], m[5]);
    g_b3(v, 3, 7, 11, 15, m[6], m[7]);
    g_b3(v, 0, 5, 10, 15, m[8], m[9]);
    g_b3(v, 1, 6, 11, 12, m[10], m[11]);
    g_b3(v, 2, 7, 8, 13, m[12], m[13]);
    g_b3(v, 3, 4, 9, 14, m[14], m[15]);
}

fn compress_b3(cv: &[u32; 8], block: &[u32; 16], counter: u64, block_len: u32, flags: u32) -> [u32; 16] {
    let mut v = [0u32; 16];
    for i in 0..8 {
        v[i] = cv[i];
        v[8 + i] = B3_IV[i];
    }
    v[12] = counter as u32;
    v[13] = (counter >> 32) as u32;
    v[14] = block_len;
    v[15] = flags;
    let mut m = *block;
    round_b3(&mut v, &m);
    for _ in 1..7 {
        let mut p = [0u32; 16];
        for i in 0..16 {
            p[i] = m[B3_MSG_PERMUTATION[i]];
        }
        m = p;
        round_b3(&mut v, &m);
    }
    let mut out = [0u32; 16];
    for i in 0..8 {
        out[i] = v[i] ^ v[i + 8];
    }
    out
}

fn first8_b3(compression_output: &[u32; 16]) -> [u32; 8] {
    let mut cv = [0u32; 8];
    for i in 0..8 {
        cv[i] = compression_output[i];
    }
    cv
}

// The state just before choosing between a chaining value and root output bytes.
#[derive(Clone, Copy)]
struct B3Out {
    cv: [u32; 8],
    block: [u32; 16],
    counter: u64,
    block_len: u32,
    flags: u32,
}

fn b3_cv_of(o: &B3Out) -> [u32; 8] {
    first8_b3(&compress_b3(&o.cv, &o.block, o.counter, o.block_len, o.flags))
}

// Output of one chunk: the last block keeps its real length and CHUNK_END.
fn b3_chunk_out(chunk: &[u8], chunk_index: u64) -> B3Out {
    let mut cv = B3_IV;
    let mut pos = 0;
    let mut blocks = 0;
    while pos + 64 < chunk.len() {
        let flags = if blocks == 0 { CHUNK_START } else { 0 };
        cv = first8_b3(&compress_b3(&cv, &b3_words(&chunk[pos..pos + 64]), chunk_index, 64, flags));
        blocks += 1;
        pos += 64;
    }
    let start = if blocks == 0 { CHUNK_START } else { 0 };
    B3Out {
        cv,
        block: b3_words(&chunk[pos..]),
        counter: chunk_index,
        block_len: (chunk.len() - pos) as u32,
        flags: start | CHUNK_END,
    }
}

// The tree: the left subtree holds the largest power of two leaves below the total.
fn b3_tree(outs: &[B3Out]) -> B3Out {
    if outs.len() == 1 {
        return outs[0];
    }
    let mut n = 1;
    while n * 2 < outs.len() {
        n *= 2;
    }
    let left = b3_tree(&outs[..n]);
    let right = b3_tree(&outs[n..]);
    let lcv = b3_cv_of(&left);
    let rcv = b3_cv_of(&right);
    let mut block = [0u32; 16];
    for i in 0..8 {
        block[i] = lcv[i];
        block[8 + i] = rcv[i];
    }
    B3Out {
        cv: B3_IV,
        block,
        counter: 0,
        block_len: 64,
        flags: PARENT,
    }
}

pub fn blake3(data: &[u8], out_bytes: usize) -> Vec<u8> {
    let out_bytes = out_bytes.min(32);
    let root = if data.len() <= 1024 {
        b3_chunk_out(data, 0)
    } else {
        let outs: Vec<B3Out> = data
            .chunks(1024)
            .enumerate()
            .map(|(i, c)| b3_chunk_out(c, i as u64))
            .collect();
        b3_tree(&outs)
    };
    let words = compress_b3(&root.cv, &root.block, root.counter, root.block_len, root.flags | ROOT);
    let mut res = Vec::new();
    for w in words.iter() {
        for k in 0..4 {
            res.push(((w >> (k * 8)) & 0xFF) as u8);
        }
    }
    res.truncate(out_bytes);
    res
}

// ---------------------------------------------------------------- SM3

// SM3 (GM/T 0004-2012). Structure follows the OpenSSL/Botan reference:
// rotating register view, verbatim K table, and a 16-slot window that is
// expanded in place after every round (not only after round 15).
const SM3_IV: [u32; 8] = [
    0x7380166f, 0x4914b2b9, 0x172442d7, 0xda8a0600, 0xa96f30bc, 0x163138aa, 0xe38dee4d, 0xb0fb0e4e,
];
const SM3_K: [u32; 64] = [
    0x79cc4519, 0xf3988a32, 0xe7311465, 0xce6228cb, 0x9cc45197, 0x3988a32f, 0x7311465e, 0xe6228cbc,
    0xcc451979, 0x988a32f3, 0x311465e7, 0x6228cbce, 0xc451979c, 0x88a32f39, 0x11465e73, 0x228cbce6,
    0x9d8a7a87, 0x3b14f50f, 0x7629ea1e, 0xec53d43c, 0xd8a7a879, 0xb14f50f3, 0x629ea1e7, 0xc53d43ce,
    0x8a7a879d, 0x14f50f3b, 0x29ea1e76, 0x53d43cec, 0xa7a879d8, 0x4f50f3b1, 0x9ea1e762, 0x3d43cec5,
    0x7a879d8a, 0xf50f3b14, 0xea1e7629, 0xd43cec53, 0xa879d8a7, 0x50f3b14f, 0xa1e7629e, 0x43cec53d,
    0x879d8a7a, 0x0f3b14f5, 0x1e7629ea, 0x3cec53d4, 0x79d8a7a8, 0xf3b14f50, 0xe7629ea1, 0xcec53d43,
    0x9d8a7a87, 0x3b14f50f, 0x7629ea1e, 0xec53d43c, 0xd8a7a879, 0xb14f50f3, 0x629ea1e7, 0xc53d43ce,
    0x8a7a879d, 0x14f50f3b, 0x29ea1e76, 0x53d43cec, 0xa7a879d8, 0x4f50f3b1, 0x9ea1e762, 0x3d43cec5,
];
const SM3_VIEW: [[usize; 4]; 4] = [[0, 1, 2, 3], [3, 0, 1, 2], [2, 3, 0, 1], [1, 2, 3, 0]];

fn sm3_p0(x: u32) -> u32 {
    x ^ x.rotate_left(9) ^ x.rotate_left(17)
}

fn sm3_p1(x: u32) -> u32 {
    x ^ x.rotate_left(15) ^ x.rotate_left(23)
}

pub fn sm3(data: &[u8]) -> Vec<u8> {
    let mut h = SM3_IV;
    let mut buf = data.to_vec();
    buf.push(0x80);
    while buf.len() % 64 != 56 {
        buf.push(0);
    }
    let bits = (data.len() as u64) * 8;
    for k in (0..8).rev() {
        buf.push(((bits >> (k * 8)) & 0xFF) as u8);
    }
    for block in buf.chunks(64) {
        let mut w = [0u32; 16];
        for i in 0..16 {
            let j = i * 4;
            w[i] = ((block[j] as u32) << 24)
                | ((block[j + 1] as u32) << 16)
                | ((block[j + 2] as u32) << 8)
                | block[j + 3] as u32;
        }
        let mut x = h[0..4].to_vec();
        let mut y = h[4..8].to_vec();
        for i in 0..64 {
            let v = SM3_VIEW[i % 4];
            let a = [x[v[0]], x[v[1]], x[v[2]], x[v[3]]];
            let e = [y[v[0]], y[v[1]], y[v[2]], y[v[3]]];
            let wi = w[i % 16];
            let wj = wi ^ w[(i + 4) % 16];
            let a12 = a[0].rotate_left(12);
            let ss1 = a12.wrapping_add(e[0]).wrapping_add(SM3_K[i]).rotate_left(7);
            let ff = if i < 16 {
                a[0] ^ a[1] ^ a[2]
            } else {
                (a[0] & a[1]) | ((a[0] | a[1]) & a[2])
            };
            let gg = if i < 16 {
                e[0] ^ e[1] ^ e[2]
            } else {
                e[2] ^ (e[0] & (e[1] ^ e[2]))
            };
            let tt1 = ff.wrapping_add(a[3]).wrapping_add(ss1 ^ a12).wrapping_add(wj);
            let tt2 = gg.wrapping_add(e[3]).wrapping_add(ss1).wrapping_add(wi);
            x[v[1]] = a[1].rotate_left(9);
            x[v[3]] = tt1;
            y[v[1]] = e[1].rotate_left(19);
            y[v[3]] = sm3_p0(tt2);
            w[i % 16] = sm3_p1(w[i % 16] ^ w[(i + 7) % 16] ^ w[(i + 13) % 16].rotate_left(15))
                ^ w[(i + 3) % 16].rotate_left(7)
                ^ w[(i + 10) % 16];
        }
        for i in 0..4 {
            h[i] ^= x[i];
            h[4 + i] ^= y[i];
        }
    }
    let mut out = Vec::new();
    for v in h {
        out.extend(u32be(v));
    }
    out
}

// ---------------------------------------------------------------- padding helper

// Yields message blocks of `size` bytes with the length appended as `len_field` bytes.
// `little_endian_length` selects MD5 (64-bit little-endian) vs SHA-1/SHA-2 (big-endian).
fn blocks(data: &[u8], size: usize, little_endian_length: bool) -> Vec<Vec<u8>> {
    let mut buf = data.to_vec();
    buf.push(0x80);
    let len_field = if size == 64 && little_endian_length { 8 } else { 8 };
    while buf.len() % size != size - len_field {
        buf.push(0);
    }
    let bits = (data.len() as u64) * 8;
    if little_endian_length {
        for k in 0..8 {
            buf.push(((bits >> (k * 8)) & 0xFF) as u8);
        }
    } else {
        for k in (0..8).rev() {
            buf.push(((bits >> (k * 8)) & 0xFF) as u8);
        }
    }
    buf.chunks(size).map(|c| c.to_vec()).collect()
}

// ================================================================ applet 行為
//
// 行為模型來自 docs/man/cksum.1.gz（+ md5sum/sha*sum/b2sum/sum 的 mirror）。
// man page 沒寫的部分（輸出欄位格式、檔名轉義、--debug 訊息）記在 docs/assumptions.md。

pub struct Opts {
    pub check: bool,
    pub warn: bool,
    pub status: bool,
    pub quiet: bool,
    pub ignore_missing: bool,
    pub strict: bool,
    pub length: usize,
    pub tag: bool,
    pub raw: bool,
    pub base64: bool,
    pub zero: bool,
    pub debug: bool,
    pub no_names: bool,
    pub algorithm: Option<&'static Algo>,
}

// 可變長度的演算法：-l 只對這些有意義。
fn variable_length(name: &str) -> bool {
    matches!(name, "blake2b" | "sha2" | "sha3" | "blake3" | "shake128" | "shake256")
}

// -a 的合法值 = cksum man page 的 DIGEST 清單。
pub fn algo_by_name(name: &str) -> Option<&'static Algo> {
    match name {
        "sysv" => Some(&SYSV),
        "bsd" => Some(&BSD),
        "crc" => Some(&CRC),
        "crc32b" => Some(&CRC32B),
        "md5" => Some(&MD5),
        "sha1" => Some(&SHA1),
        "sha2" => Some(&SHA2),
        "sha3" => Some(&SHA3),
        "blake2b" => Some(&BLAKE2B),
        "sm3" => Some(&SM3),
        "sha224" => Some(&SHA224),
        "sha256" => Some(&SHA256),
        "sha384" => Some(&SHA384),
        "sha512" => Some(&SHA512),
        "blake3" => Some(&BLAKE3),
        "shake128" => Some(&SHAKE128),
        "shake256" => Some(&SHAKE256),
        _ => None,
    }
}

// man page 沒寫 hashsum 的預設演算法 → 視為未定義（見 docs/assumptions.md）。
pub fn hashsum_needs_algorithm(applet: &str, sel: Option<&'static Algo>) -> bool {
    applet == "hashsum" && sel.is_none()
}

// tagged 輸出裡的名稱：sha2/sha3/shake 的顯示名含輸出長度（實測參考 binary：SHA256、SHA3-256、SHAKE128-64）。
fn display_name(algo: &Algo, opts: &Opts) -> String {
    let n = if opts.length != 0 { opts.length } else { algo.bits };
    match algo.name {
        "sha2" => format!("SHA{}", n),
        "sha3" => format!("SHA3-{}", n),
        "shake128" => format!("SHAKE128-{}", n),
        "shake256" => format!("SHAKE256-{}", n),
        // blake2b/blake3：只有明確給了 -l 且不等於預設長度才有後綴（實測：-l 512 → BLAKE2b，-l 256 → BLAKE2b-256）。
        "blake2b" | "blake3" => {
            if opts.length != 0 && opts.length != algo.bits {
                format!("{}-{}", algo.display, opts.length)
            } else {
                algo.display.to_string()
            }
        }
        _ => algo.display.to_string(),
    }
}

pub fn hex_of(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

fn b64_of(bytes: &[u8]) -> String {
    const A: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut s = String::new();
    for chunk in bytes.chunks(3) {
        let n = chunk.len();
        let v = (chunk[0] as u32) << 16
            | (if n > 1 { chunk[1] as u32 } else { 0 }) << 8
            | (if n > 2 { chunk[2] as u32 } else { 0 });
        s.push(A[((v >> 18) & 63) as usize] as char);
        s.push(A[((v >> 12) & 63) as usize] as char);
        s.push(if n > 1 { A[((v >> 6) & 63) as usize] as char } else { '=' });
        s.push(if n > 2 { A[(v & 63) as usize] as char } else { '=' });
    }
    s
}

fn esc(name: &str, zero: bool) -> String {
    if zero {
        return name.to_string();
    }
    let mut s = String::new();
    for c in name.chars() {
        if c == '\\' {
            s.push_str("\\\\");
        } else if c == '\n' {
            s.push_str("\\n");
        } else {
            s.push(c);
        }
    }
    s
}

use std::io::Read;
use std::io::Write;

fn errline(stderr: &mut dyn Write, applet: &str, msg: &str) {
    let _ = stderr.write_all(format!("{}: {}\n", applet, msg).as_bytes());
}

fn out_line(stdout: &mut dyn Write, s: &str) {
    let _ = stdout.write_all(s.as_bytes());
}

struct Parsed {
    opts: Opts,
    files: Vec<String>,
}

fn parse(algo: &Algo, applet: &str, args: &[String], default_tag: bool, stdout: &mut dyn Write, stderr: &mut dyn Write) -> Result<Parsed, i32> {
    let mut o = Opts {
        check: false,
        warn: false,
        status: false,
        quiet: false,
        ignore_missing: false,
        strict: false,
        length: 0,
        tag: default_tag,
        raw: false,
        base64: false,
        zero: false,
        debug: false,
        no_names: false,
        algorithm: None,
    };
    let mut files = Vec::new();
    let mut no_more = false;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].clone();
        if no_more || (!a.starts_with('-') && a != "-") {
            files.push(a);
            i += 1;
            continue;
        }
        if a == "--" {
            no_more = true;
            i += 1;
            continue;
        }
        if a == "-" {
            files.push(a);
            i += 1;
            continue;
        }
        if a == "-h" || a == "--help" {
            out_line(stdout, &format!("{}: print or check checksums\n", applet));
            return Ok(Parsed { opts: o, files });
        }
        if a == "-V" || a == "--version" {
            out_line(stderr, &format!("{} (uutils coreutils) 0.10.0\n", applet));
            return Ok(Parsed { opts: o, files });
        }
        if a == "--no-names" && is_hashsum_template(applet) {
            o.no_names = true;
            i += 1;
            continue;
        }
        if (a == "-b" || a == "--binary") && is_hashsum_template(applet) {
            i += 1;
            continue;
        }
        if is_hashsum_template(applet) {
            if let Some(al) = hashsum_selector(&a) {
                o.algorithm = Some(al);
                i += 1;
                continue;
            }
        }
        let attached = a.len() > 2 && a.starts_with('-') && !a.starts_with("--");
        let flag = if attached { a[0..2].to_string() } else { a.clone() };
        let value = if attached {
            a[2..].to_string()
        } else if i + 1 < args.len() {
            args[i + 1].clone()
        } else {
            String::new()
        };
        if !option_allowed(applet, &flag) {
            errline(stderr, applet, &format!("invalid option -- '{}'", a[1..].to_string()));
            return Err(1);
        }
        match flag.as_str() {
            "-c" | "--check" => o.check = true,
            "-w" | "--warn" => o.warn = true,
            "--status" => o.status = true,
            "--quiet" => o.quiet = true,
            "--ignore-missing" => o.ignore_missing = true,
            "--strict" => o.strict = true,
            "--tag" => o.tag = true,
            "--untagged" => o.tag = false,
            "--raw" => o.raw = true,
            "--base64" => o.base64 = true,
            "-z" | "--zero" => o.zero = true,
            "-t" | "--text" => {}
            "--debug" => o.debug = true,
            "-a" | "--algorithm" => {
                let al = algo_by_name(&value);
                if al.is_none() {
                    errline(stderr, applet, &format!("invalid algorithm: '{}'", value));
                    return Err(1);
                }
                o.algorithm = al;
                if !attached {
                    i += 1;
                }
            }
            "--bits" if is_hashsum_template(applet) => {
                let n: usize = match value.parse::<usize>() {
                    Ok(v) => v,
                    Err(_) => {
                        errline(stderr, applet, &format!("invalid length: '{}'", value));
                        return Err(1);
                    }
                };
                o.length = n;
                if !attached {
                    i += 1;
                }
            }
            "-r" if applet == "sum" => o.algorithm = Some(&BSD),
            "-s" | "--sysv" if applet == "sum" => o.algorithm = Some(&SYSV),
            "-l" | "--length" => {
                let resolved_name = o.algorithm.unwrap_or(algo).name;
                if !variable_length(resolved_name) {
                    errline(stderr, applet, "--length is only supported with --algorithm blake2b, sha2, or sha3");
                    return Err(1);
                }
                let n: usize = match value.parse::<usize>() {
                    Ok(v) => v,
                    Err(_) => {
                        errline(stderr, applet, &format!("invalid length: '{}'", value));
                        return Err(1);
                    }
                };
                if n != 0 {
                    // reference prints "invalid length: '<raw>'" then the reason on its own line
                    let bad = if matches!(resolved_name, "sha2" | "sha3") {
                        !matches!(n, 224 | 256 | 384 | 512)
                    } else if resolved_name == "blake2b" {
                        n % 8 != 0 || n > 512
                    } else if resolved_name == "blake3" {
                        n % 8 != 0
                    } else {
                        false // shake accepts any length (verified against the reference)
                    };
                    if bad {
                        errline(stderr, applet, &format!("invalid length: '{}'", value));
                        if matches!(resolved_name, "sha2" | "sha3") {
                            errline(stderr, applet, &format!("digest length for '{}' must be 224, 256, 384, or 512", display_of(resolved_name)));
                        } else if resolved_name == "blake2b" && n > 512 {
                            errline(stderr, applet, &format!("maximum digest length for '{}' is 512 bits", display_of(resolved_name)));
                        } else {
                            errline(stderr, applet, "length is not a multiple of 8");
                        }
                        return Err(1);
                    }
                }
                o.length = n;
                if !attached {
                    i += 1;
                }
            }
            _ => {
                errline(stderr, applet, &format!("invalid option -- '{}'", if attached { a[1..].to_string() } else { a[1..].to_string() }));
                return Err(1);
            }
        }
        i += 1;
    }
    if o.length != 0 && applet != "cksum" && applet != "b2sum" && !matches!(o.algorithm.unwrap_or(algo).name, "shake128" | "shake256") {
        errline(stderr, applet, "--bits is only supported with --algorithm shake128 or shake256");
        return Err(1);
    }
    Ok(Parsed { opts: o, files })
}

// hashsum man page 的演算法 selector（最後出現的 wins，與 basenc 的 selector 相同規則）。
fn hashsum_selector(a: &str) -> Option<&'static Algo> {
    match a {
        "--md5" => Some(&MD5),
        "--sha1" => Some(&SHA1),
        "--sha224" => Some(&SHA224),
        "--sha256" => Some(&SHA256),
        "--sha384" => Some(&SHA384),
        "--sha512" => Some(&SHA512),
        "--sha3" => Some(&SHA3),
        "--sha3-224" => Some(&SHA3_224),
        "--sha3-256" => Some(&SHA3_256),
        "--sha3-384" => Some(&SHA3_384),
        "--sha3-512" => Some(&SHA3_512),
        "--shake128" => Some(&SHAKE128),
        "--shake256" => Some(&SHAKE256),
        "--b2sum" => Some(&BLAKE2B),
        "--b3sum" => Some(&BLAKE3),
        _ => None,
    }
}

// man page mirror 為空的 applet（sha3-*/shake*/b3sum/hashsum）：行為由 hashsum 的 man page 推導。
fn is_hashsum_template(applet: &str) -> bool {
    matches!(applet, "hashsum" | "sha3sum" | "sha3-224sum" | "sha3-256sum" | "sha3-384sum" | "sha3-512sum" | "shake128sum" | "shake256sum" | "b3sum")
}

fn display_of(name: &str) -> String {
    algo_by_name(name).map(|a| a.display.to_string()).unwrap_or_else(|| name.to_string())
}

// 每個 applet 接受的選項 = 它自己 man page 的 SYNOPSIS。man page 沒列的不接受。
fn option_allowed(applet: &str, flag: &str) -> bool {
    if flag == "-h" || flag == "--help" || flag == "-V" || flag == "--version" {
        return true;
    }
    if applet == "sum" {
        return matches!(flag, "-r" | "-s" | "--sysv");
    }
    let base = matches!(flag, "-c" | "--check" | "-w" | "--warn" | "--status" | "--quiet" | "--ignore-missing" | "--strict" | "--tag" | "-t" | "--text" | "-z" | "--zero");
    match applet {
        "cksum" => base || matches!(flag, "--untagged" | "-l" | "--length" | "--raw" | "--base64" | "--debug" | "-a" | "--algorithm"),
        // sha3-*/shake*/b3sum 的 man page mirror 是空的（見 docs/assumptions.md），
        // 選項集合由 hashsum 的 man page 推導（同一個模板、固定演算法）。
        _ if is_hashsum_template(applet) => base || matches!(flag, "-b" | "--binary" | "--bits" | "--no-names") || hashsum_selector(flag).is_some(),
        "b2sum" => base || matches!(flag, "-l" | "--length"),
        _ => base,
    }
}



fn read_all(path: &str, stdin: &mut dyn Read) -> Option<Vec<u8>> {
    if path == "-" {
        let mut v = Vec::new();
        if stdin.read_to_end(&mut v).is_err() {
            return None;
        }
        return Some(v);
    }
    std::fs::read(path).ok()
}

fn digest_name(algo: &Algo, opts: &Opts) -> String {
    display_name(algo, opts)
}

/// 印出一列：tagged = `NAME (file) = digest`，untagged = `digest  file`。
fn format_line(algo: &Algo, opts: &Opts, file: &str, data: &[u8]) -> String {
    let nl = if opts.zero { "\0" } else { "\n" };
    let o_no_names = opts.no_names;
    match algo.name {
        "sysv" | "bsd" | "crc" | "crc32b" => {
            let v = digest(algo, data, 0);
            let n = u32from(&v);
            let label = if file == "-" || o_no_names { String::new() } else { format!(" {}", esc(file, opts.zero)) };
            match algo.name {
                "bsd" => format!("{:05} {:5}{}{}", n, blocks_of(data.len(), 1024), label, nl),
                "sysv" => format!("{} {}{}{}", n, blocks_of(data.len(), 512), label, nl),
                _ => format!("{} {}{}{}", n, data.len(), label, nl),
            }
        }
        _ => {
            let d = digest(algo, data, opts.length);
            let shown = if opts.raw {
                String::from_utf8_lossy(&d).to_string()
            } else if opts.base64 {
                b64_of(&d)
            } else {
                hex_of(&d)
            };
            let f = if file == "-" { "-".to_string() } else { esc(file, opts.zero) };
            if opts.no_names || opts.raw {
                return format!("{}{}", shown, nl);
            }
            if opts.tag {
                format!("{} ({}) = {}{}", digest_name(algo, opts), f, shown, nl)
            } else {
                format!("{}  {}{}", shown, f, nl)
            }
        }
    }
}

fn blocks_of(len: usize, block: usize) -> usize {
    if len == 0 {
        return 0;
    }
    (len + block - 1) / block
}

fn u32from(b: &[u8]) -> u32 {
    (b[0] as u32) | ((b[1] as u32) << 8) | ((b[2] as u32) << 16) | ((b[3] as u32) << 24)
}

pub fn run(
    algo: &Algo,
    applet: &str,
    args: &[String],
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    default_tag: bool,
) -> i32 {
    let parsed = match parse(algo, applet, args, default_tag, stdout, stderr) {
        Ok(p) => p,
        Err(r) => return r,
    };
    let o = &parsed.opts;
    let algo = o.algorithm.unwrap_or(algo);
    if applet == "cksum" && matches!(algo.name, "sha2" | "sha3") && o.length == 0 {
        errline(stderr, applet, &format!("--algorithm={} requires specifying --length 224, 256, 384, or 512", algo.name));
        return 1;
    }
    if hashsum_needs_algorithm(applet, o.algorithm) {
        errline(stderr, applet, "no algorithm specified");
        return 1;
    }
    // 只有「明確指定」數值型演算法時才拒絕 -c；未指定時由每一行的 tag 名稱推導（GNU/uutils 行為）。
    if o.check && o.algorithm.is_some() && matches!(algo.name, "sysv" | "bsd" | "crc" | "crc32b") {
        errline(stderr, applet, "--check is not supported with --algorithm={bsd,sysv,crc,crc32b}");
        return 1;
    }
    if o.check && o.tag && default_tag == false {
        errline(stderr, applet, "the --tag option is meaningless when verifying checksums");
        return 1;
    }
    if o.debug {
        let _ = stderr.write_all(format!("{}: no hardware acceleration detected\n", applet).as_bytes());
    }
    let files = if parsed.files.is_empty() { vec!["-".to_string()] } else { parsed.files.clone() };
    let mut rc = 0;
    for f in &files {
        let data = match read_all(f, stdin) {
            Some(d) => d,
            None => {
                errline(stderr, applet, &format!("{}: No such file or directory", f));
                rc = 1;
                continue;
            }
        };
        if o.check {
            rc = rc.max(check_one(algo, applet, o, f, &data, stdout, stderr));
        } else {
            let line = format_line(algo, o, f, &data);
            if stdout.write_all(line.as_bytes()).is_err() {
                return 1;
            }
        }
    }
    rc
}

fn check_one(
    algo: &Algo,
    applet: &str,
    o: &Opts,
    src: &str,
    content: &[u8],
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let want_bits = if o.length != 0 { o.length } else { algo.bits };
    let want_len = want_bits / 8;
    let mut found = false;
    let mut bad = false;
    let mut rc = 0;
    let mut n_improper = 0usize;
    let mut n_mismatch = 0usize;
    let mut n_unread = 0usize;
    let mut verified = 0usize;
    for (lineno, line) in content.split(|&b| b == b'\n').enumerate() {
        if line.is_empty() {
            continue;
        }
        let s = String::from_utf8_lossy(line).to_string();
        let (line_algo, line_len, digest_hex, target) = match parse_line(&s, want_len) {
            Some(x) => x,
            None => {
                n_improper += 1;
                if o.warn {
                    errline(stderr, applet, &format!("{}: {}: improperly formatted {} checksum line", src, lineno + 1, digest_name(algo, o)));
                }
                bad = true;
                continue;
            }
        };
        found = true;
        let data = match std::fs::read(&target) {
            Ok(d) => d,
            Err(_) => {
                if o.ignore_missing {
                    continue;
                }
                errline(stderr, applet, &format!("{}: No such file or directory", target));
                out_line(stdout, &format!("{}: FAILED open or read\n", target));
                n_unread += 1;
                rc = 1;
                continue;
            }
        };
        let a = line_algo.unwrap_or(algo);
        let bits = if line_len != 0 { line_len * 8 } else { o.length };
        let got = digest(a, &data, bits);
        verified += 1;
        if got == digest_hex {
            if !o.quiet && !o.status {
                out_line(stdout, &format!("{}: OK\n", target));
            }
        } else {
            n_mismatch += 1;
            // --status 連 FAILED 也抑制（實測參考：只有 open or read 會印出）
            if !o.status {
                out_line(stdout, &format!("{}: FAILED\n", target));
            }
            rc = 1;
        }
    }
    if !found {
        errline(stderr, applet, &format!("{}: no properly formatted checksum lines found", src));
        rc = 1;
    } else if o.ignore_missing && verified == 0 {
        errline(stderr, applet, &format!("{}: no file was verified", src));
        rc = 1;
    } else if !o.status {
        // GNU 的摘要警告順序：格式錯誤 → 不符 → 無法讀取
        if n_improper > 0 {
            errline(stderr, applet, &format!("WARNING: {} {} improperly formatted", n_improper, if n_improper == 1 { "line is" } else { "lines are" }));
        }
        if n_mismatch > 0 {
            errline(stderr, applet, &format!("WARNING: {} computed checksum did NOT match", n_mismatch));
        }
        if n_unread > 0 {
            errline(stderr, applet, &format!("WARNING: {} {} could not be read", n_unread, if n_unread == 1 { "listed file" } else { "listed files" }));
        }
    }
    if bad && o.strict {
        rc = 1;
    }
    rc
}

// 支援兩種格式：tagged `NAME (file) = digest` 與 untagged `digest  file`（或 `digest *file`）。
fn parse_line(s: &str, want_len: usize) -> Option<(Option<&'static Algo>, usize, Vec<u8>, String)> {
    if let Some(rest) = s.split_once(") = ") {
        let d = hex_to_bytes(rest.1)?;
        let inner = rest.0;
        let (name, file) = inner.split_once('(')?;
        let named = algo_from_tag(name);
        let len = match named {
            Some((a, l)) => if l != 0 { l } else { a.bits / 8 },
            None => want_len,
        };
        if d.len() != len {
            return None;
        }
        return Some((named.map(|(a, _)| a), len, d, unescape(file)));
    }
    let (dpart, rest) = s.split_once("  ").or_else(|| s.split_once(" *"))?;
    let d = hex_to_bytes(dpart)?;
    if d.len() != want_len {
        return None;
    }
    Some((None, want_len, d, unescape(rest)))
}

// check mode 可以從 tagged 行的演算法名稱推導出 digest（GNU/uutils 行為：cksum -c 讀到
// `SHA256 (file) = hex` 時用 SHA256，即使 applet 預設是 crc）。
fn algo_from_tag(tag: &str) -> Option<(&'static Algo, usize)> {
    let t = tag.trim().to_lowercase();
    let (base, len) = match t.split_once('-') {
        Some((b, n)) if !n.is_empty() && n.chars().all(|c| c.is_ascii_digit()) => (b, n.parse::<usize>().ok().unwrap_or(0)),
        _ => (t.as_str(), 0),
    };
    let a = algo_by_name(base)?;
    Some((a, len))
}

fn hex_to_bytes(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    let mut v = Vec::new();
    for chunk in s.as_bytes().chunks(2) {
        let hi = (chunk[0] as char).to_digit(16)?;
        let lo = (chunk[1] as char).to_digit(16)?;
        v.push(((hi << 4) | lo) as u8);
    }
    Some(v)
}

fn unescape(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars();
    while let Some(c) = it.next() {
        if c == '\\' {
            match it.next() {
                Some('n') => out.push('\n'),
                Some('\\') => out.push('\\'),
                Some(x) => {
                    out.push('\\');
                    out.push(x);
                }
                None => out.push('\\'),
            }
        } else {
            out.push(c);
        }
    }
    out
}
