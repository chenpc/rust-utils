// Shared base-N encoding core for base32, base64 and basenc.
//
// Behavior model (docs/man/base64.1.gz, docs/man/base32.1.gz, docs/man/basenc.1.gz):
//   encode: group input bytes into bit groups of `bits` bits, output alphabet chars;
//   padding char '=' completes the last group (no padding for base16/z85/base58);
//   decode: newlines are always allowed; without --ignore-garbage a byte outside the
//   alphabet is an error (exit 1), with -i it is skipped;
//   -w COLS wraps encoded output after COLS chars (default 76, 0 disables wrapping);
//   --z85: encoding requires input length multiple of 4, decoding requires multiple of 5;
//   --base58: the whole input is one big integer; leading zero bytes become '1';
//   --base2lsbf outputs bits least significant first, --base2msbf most significant first.
//
// Assumptions (docs/assumptions.md):
//   - the error text for invalid input is implementation-defined; exit status 1;
//   - wrapping inserts '\n' after every COLS chars of output.

use std::io::{Read, Write};

#[derive(PartialEq)]
pub enum Kind {
    Group,
    Z85,
    Base58,
}

pub struct Codec {
    pub name: &'static str,
    pub alphabet: &'static [u8],
    pub bits: usize,
    pub pad: bool,
    pub lsb: bool,
    pub kind: Kind,
}

pub fn encode(codec: &Codec, data: &[u8], wrap: usize, stderr: &mut dyn Write) -> Result<Vec<u8>, i32> {
    if codec.kind == Kind::Z85 && data.len() % 4 != 0 {
        let _ = stderr.write_all(format!("{}: input length must be a multiple of 4\n", codec.name).as_bytes());
        return Err(1);
    }
    let out = match codec.kind {
        Kind::Group if codec.lsb => {
            let mut v = Vec::new();
            for &b in data {
                for i in 0..8 {
                    v.push(codec.alphabet[((b >> i) & 1) as usize]);
                }
            }
            v
        }
        Kind::Group => {
            let gb = group_bytes(codec.bits);
            let gc = 8 * gb / codec.bits;
            let mut v = Vec::new();
            for chunk in data.chunks(gb) {
                let mut acc: u64 = 0;
                for &b in chunk {
                    acc = (acc << 8) | b as u64;
                }
                let full = chunk.len() * 8;
                let produced = (full + codec.bits - 1) / codec.bits;
                for i in 0..produced {
                    let shift = full as isize - ((i + 1) * codec.bits) as isize;
                    let val = if shift >= 0 {
                        (acc >> shift) & ((1u64 << codec.bits) - 1)
                    } else {
                        // the last char of a partial group uses the remaining bits,
                        // shifted left to fill a full group value
                        let avail = full - i * codec.bits;
                        (acc & ((1u64 << avail) - 1)) << (-shift)
                    };
                    v.push(codec.alphabet[val as usize]);
                }
                if codec.pad && chunk.len() < gb {
                    for _ in produced..gc {
                        v.push(b'=');
                    }
                }
            }
            v
        }
        Kind::Z85 => {
            let mut v = Vec::new();
            for chunk in data.chunks(4) {
                if chunk.len() != 4 {
                    continue;
                }
                // base-85 digits, most significant first
                let n = ((chunk[0] as u32) << 24)
                    | ((chunk[1] as u32) << 16)
                    | ((chunk[2] as u32) << 8)
                    | chunk[3] as u32;
                for i in (0..5).rev() {
                    let mut p: u64 = 1;
                    for _ in 0..i {
                        p *= 85;
                    }
                    v.push(codec.alphabet[((n as u64 / p) % 85) as usize]);
                }
            }
            v
        }
        Kind::Base58 => {
            let mut v: Vec<u8> = Vec::new();
            let zeros = data.iter().take_while(|&&b| b == 0).count();
            let mut digits: Vec<u8> = Vec::new();
            let mut num: Vec<u32> = data[zeros..].iter().map(|&b| b as u32).collect();
            while !num.is_empty() {
                let mut rem: u32 = 0;
                for limb in num.iter_mut() {
                    let cur = rem * 256 + *limb;
                    *limb = cur / 58;
                    rem = cur % 58;
                }
                digits.push(codec.alphabet[rem as usize]);
                while num.first() == Some(&0) {
                    num.remove(0);
                }
            }
            digits.reverse();
            for _ in 0..zeros {
                v.push(codec.alphabet[0]);
            }
            v.extend(digits);
            v
        }
    };
    if wrap > 0 && !out.is_empty() {
        let mut wrapped = Vec::with_capacity(out.len() + out.len() / wrap);
        for (i, c) in out.iter().enumerate() {
            if i > 0 && i % wrap == 0 {
                wrapped.push(b'\n');
            }
            wrapped.push(*c);
        }
        if out.len() % wrap != 0 {
            wrapped.push(b'\n');
        }
        return Ok(wrapped);
    }
    Ok(out)
}

fn group_bytes(bits: usize) -> usize {
    match bits {
        6 => 3,
        5 => 5,
        4 => 1,
        1 => 1,
        _ => 1,
    }
}

pub fn decode_bytes(codec: &Codec, data: &[u8], ignore_garbage: bool, stderr: &mut dyn Write) -> Result<Vec<u8>, i32> {
    // newlines are always ignored; non-alphabet bytes are either skipped (-i) or an error.
    // Filtering before grouping matters: a skipped byte must not consume a bit slot.
    let mut chars: Vec<u8> = Vec::new();
    for &b in data {
        if b == b'\n' || b == b'\r' {
            continue;
        }
        if codec.kind == Kind::Group && b == b'=' {
            chars.push(b);
        } else if codec.alphabet.contains(&b) {
            chars.push(b);
        } else if ignore_garbage {
            continue;
        } else {
            let _ = stderr.write_all(format!("{}: invalid input\n", codec.name).as_bytes());
            return Err(1);
        }
    }
    match codec.kind {
        Kind::Z85 => {
            if chars.len() % 5 != 0 {
                let _ = stderr.write_all(format!("{}: input length must be a multiple of 5\n", codec.name).as_bytes());
                return Err(1);
            }
            let mut out = Vec::new();
            for chunk in chars.chunks(5) {
                let mut n: u64 = 0;
                for &c in chunk {
                    let idx = match codec.alphabet.iter().position(|&a| a == c) {
                        Some(i) => i,
                        None => {
                            if ignore_garbage {
                                continue;
                            }
                            let _ = stderr.write_all(format!("{}: invalid input\n", codec.name).as_bytes());
                            return Err(1);
                        }
                    };
                    n = n * 85 + idx as u64;
                }
                for i in (0..4).rev() {
                    out.push(((n >> (i * 8)) & 0xFF) as u8);
                }
            }
            Ok(out)
        }
        Kind::Base58 => {
            let zeros = chars.iter().take_while(|&&c| c == codec.alphabet[0]).count();
            let mut out: Vec<u8> = vec![0; zeros];
            let mut num: Vec<u32> = Vec::new();
            for &c in &chars[zeros..] {
                let idx = match codec.alphabet.iter().position(|&a| a == c) {
                    Some(i) => i,
                    None => {
                        if ignore_garbage {
                            continue;
                        }
                        let _ = stderr.write_all(format!("{}: invalid input\n", codec.name).as_bytes());
                        return Err(1);
                    }
                };
                let mut carry = idx as u64;
                for limb in num.iter_mut().rev() {
                    let cur = *limb as u64 * 58 + carry;
                    *limb = (cur & 0xFF) as u32;
                    carry = cur >> 8;
                }
                while carry > 0 {
                    num.insert(0, (carry & 0xFF) as u32);
                    carry >>= 8;
                }
            }
            out.extend(num.iter().map(|&l| l as u8));
            Ok(out)
        }
        Kind::Group => {
            let gc = 8 * group_bytes(codec.bits) / codec.bits;
            let mut out: Vec<u8> = Vec::new();
            if codec.lsb {
                for group in chars.chunks(8) {
                    let mut byte: u8 = 0;
                    for (i, &c) in group.iter().enumerate() {
                        let idx = match codec.alphabet.iter().position(|&a| a == c) {
                            Some(i) => i,
                            None => {
                                if ignore_garbage {
                                    continue;
                                }
                                let _ = stderr.write_all(format!("{}: invalid input\n", codec.name).as_bytes());
                                return Err(1);
                            }
                        };
                        byte |= (idx as u8) << i;
                    }
                    out.push(byte);
                }
                return Ok(out);
            }
            for group in chars.chunks(gc) {
                let mut valid = group.len();
                for (i, &c) in group.iter().enumerate() {
                    if c == b'=' {
                        valid = i;
                        break;
                    }
                }
                let mut n: u64 = 0;
                for &c in &group[..valid] {
                    let idx = match codec.alphabet.iter().position(|&a| a == c) {
                        Some(i) => i,
                        None => {
                            if ignore_garbage {
                                continue;
                            }
                            let _ = stderr.write_all(format!("{}: invalid input\n", codec.name).as_bytes());
                            return Err(1);
                        }
                    };
                    n = n * (1u64 << codec.bits) + idx as u64;
                }
                let bytes = valid * codec.bits / 8;
                let width = valid * codec.bits;
                for i in 0..bytes {
                    out.push(((n >> (width - (i + 1) * 8)) & 0xFF) as u8);
                }
            }
            Ok(out)
        }
    }
}

pub const BASE64: Codec = Codec {
    name: "base64",
    alphabet: b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/",
    bits: 6,
    pad: true,
    lsb: false,
    kind: Kind::Group,
};

pub const BASE32: Codec = Codec {
    name: "base32",
    alphabet: b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567",
    bits: 5,
    pad: true,
    lsb: false,
    kind: Kind::Group,
};

pub const BASE16: Codec = Codec {
    name: "base16",
    alphabet: b"0123456789ABCDEF",
    bits: 4,
    pad: false,
    lsb: false,
    kind: Kind::Group,
};

pub const BASE64URL: Codec = Codec {
    name: "base64url",
    alphabet: b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_",
    bits: 6,
    pad: true,
    lsb: false,
    kind: Kind::Group,
};

pub const BASE32HEX: Codec = Codec {
    name: "base32hex",
    alphabet: b"0123456789ABCDEFGHIJKLMNOPQRSTUV",
    bits: 5,
    pad: true,
    lsb: false,
    kind: Kind::Group,
};

pub const BASE2LSBF: Codec = Codec {
    name: "base2lsbf",
    alphabet: b"01",
    bits: 1,
    pad: false,
    lsb: true,
    kind: Kind::Group,
};

pub const BASE2MSBF: Codec = Codec {
    name: "base2msbf",
    alphabet: b"01",
    bits: 1,
    pad: false,
    lsb: false,
    kind: Kind::Group,
};

pub const Z85: Codec = Codec {
    name: "z85",
    alphabet: b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ.-:+=^!/*?&<>()[]{}@%$#",
    bits: 7,
    pad: false,
    lsb: false,
    kind: Kind::Z85,
};

pub const BASE58: Codec = Codec {
    name: "base58",
    alphabet: b"123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz",
    bits: 6,
    pad: false,
    lsb: false,
    kind: Kind::Base58,
};

// Shared option handling for the base-N applets.
pub fn run(
    codec: &Codec,
    display: &str,
    args: &[String],
    stdin: &mut dyn Read,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    default_wrap: usize,
) -> i32 {
    let mut decode = false;
    let mut ignore = false;
    let mut wrap = default_wrap;
    let mut files: Vec<String> = Vec::new();
    let mut i = 0;
    let mut no_more = false;
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
            if stdout
                .write_all(format!("{}: encode/decode data and print to standard output\n", display).as_bytes())
                .is_err()
            {
                return 1;
            }
            return 0;
        }
        if a == "-V" || a == "--version" {
            if stdout
                .write_all(format!("{} (uutils coreutils) 0.10.0\n", display).as_bytes())
                .is_err()
            {
                return 1;
            }
            return 0;
        }
        let attached = a.len() > 2 && a.starts_with('-') && !a.starts_with("--");
        let flag = if attached { a[0..2].to_string() } else { a.clone() };
        match flag.as_str() {
            "-d" | "--decode" => decode = true,
            "-i" | "--ignore-garbage" => ignore = true,
            "-w" | "--wrap" => {
                let value = if attached {
                    a[2..].to_string()
                } else if i + 1 < args.len() {
                    args[i + 1].clone()
                } else {
                    String::new()
                };
                if value.is_empty() {
                    let _ = stderr.write_all(format!("{}: missing operand\n", display).as_bytes());
                    return 2;
                }
                wrap = match value.parse::<usize>() {
                    Ok(n) => n,
                    Err(_) => {
                        let _ = stderr.write_all(format!("{}: invalid number: {}\n", display, value).as_bytes());
                        return 2;
                    }
                };
                if !attached {
                    i += 1;
                }
            }
            _ => {
                let _ = stderr.write_all(format!("{}: unknown option: {}\n", display, a).as_bytes());
                return 2;
            }
        }
        i += 1;
    }

    let data: Vec<u8> = if files.is_empty() {
        let mut buf = Vec::new();
        if stdin.read_to_end(&mut buf).is_err() {
            let _ = stderr.write_all(format!("{}: standard input: read error\n", display).as_bytes());
            return 1;
        }
        buf
    } else {
        let mut buf = Vec::new();
        let mut status = 0;
        for f in &files {
            if f == "-" {
                let mut b = Vec::new();
                if stdin.read_to_end(&mut b).is_err() {
                    status = 1;
                } else {
                    buf.extend_from_slice(&b);
                }
                continue;
            }
            match std::fs::File::open(f) {
                Ok(mut fh) => {
                    let mut b = Vec::new();
                    if fh.read_to_end(&mut b).is_err() {
                        status = 1;
                    } else {
                        buf.extend_from_slice(&b);
                    }
                }
                Err(e) => {
                    let _ = stderr.write_all(format!("{}: {}: {}\n", display, f, e).as_bytes());
                    status = 1;
                }
            }
        }
        if status != 0 {
            return status;
        }
        buf
    };

    let out = if decode {
        match decode_bytes(codec, &data, ignore, stderr) {
            Ok(v) => v,
            Err(code) => return code,
        }
    } else {
        match encode(codec, &data, wrap, stderr) {
            Ok(v) => v,
            Err(code) => return code,
        }
    };
    if stdout.write_all(&out).is_err() {
        return 1;
    }
    0
}
