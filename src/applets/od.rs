//! od — dump files in octal and other formats.
//!
//! Behavior model from docs/man/od.1.gz (uutils coreutils 0.10.0 mirror):
//! formats -a -b -c -d -D -o -I -L -i -l -x -h -O -s -X -H -e -f -F and -t TYPE;
//! -A RADIX (o,d,x,n), -j SKIP, -N BYTES, --endian big|little, -S [BYTES],
//! -v (no duplicate suppression), -w [BYTES] (32 implied without BYTES),
//! --traditional, -h, -V. Multiple formats print one line each; only the first
//! line carries the offset. A `z` suffix adds an ASCII dump at the end of the line.
//!
//! Assumptions (the man page does not fix them):
//! - default format is octal with 2-byte units, default width 8 bytes per line,
//!   offsets in octal with 7 digits;
//! - field widths: octal/hex = 2*SIZE digits (zero padded), signed/unsigned
//!   decimal = 2*SIZE+1, character/named = 3, float = 6; values are separated
//!   by one space;
//! - duplicate lines are replaced by `*` unless -v;
//! - the total byte count is printed as a final line in the offset radix.

use std::io::Read;
use crate::{In, Out};

struct Fmt {
    kind: char,
    size: usize,
    zdump: bool,
}

fn size_for(s: &str, kind: char) -> usize {
    match s {
        "C" | "1" => 1,
        "S" | "2" => 2,
        "I" | "4" => 4,
        "L" => if kind == 'f' { 16 } else { 8 },
        "F" => 4,
        "D" => 8,
        "8" => 8,
        "16" => 16,
        _ => match kind {
            'a' | 'c' => 1,
            'f' => 4,
            _ => 2,
        },
    }
}

fn parse_bytes(s: &str) -> i64 {
    let mut t = s;
    let mut mul = 1i64;
    if let Some(last) = t.chars().last() {
        let m = match last {
            'b' => 512,
            'K' => 1000,
            'M' => 1000_000,
            'G' => 1000_000_000,
            _ => 0,
        };
        if m != 0 {
            // K also means 1024 when the value is octal-style; keep the man page table.
            mul = m;
            t = &t[..t.len() - 1];
        }
    }
    let v = if t.starts_with("0x") {
        i64::from_str_radix(&t[2..], 16).unwrap_or(0)
    } else if t.starts_with('0') && t.len() > 1 {
        i64::from_str_radix(&t[1..], 8).unwrap_or(0)
    } else {
        t.parse::<i64>().unwrap_or(0)
    };
    v * mul
}

fn fmt_name(n: u8) -> String {
    const NAMES: [&str; 29] = [
        "nul", "soh", "stx", "etx", "eot", "enq", "ack", "bel", "bs", "ht", "nl", "vt", "ff", "cr",
        "so", "si", "dle", "dc1", "dc2", "dc3", "dc4", "nak", "syn", "etb", "can", "em", "sub",
        "esc", "del",
    ];
    if (n as usize) < NAMES.len() {
        NAMES[n as usize].to_string()
    } else {
        format!("{:03o}", n)
    }
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut formats: Vec<Fmt> = Vec::new();
    let mut radix = 'o';
    let mut skip = 0i64;
    let mut read_bytes = -1i64;
    let mut width = 8usize;
    let mut verbose = false;
    let mut endian = "little";
    let mut traditional = false;
    let mut strings_min = 0usize;
    let mut files: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        if no_more_opts {
            files.push(a.to_string());
            i += 1;
            continue;
        }
        if a.len() > 2 && a.starts_with('-') && !a.starts_with("--") {
            let c = a.chars().nth(1).unwrap();
            if "tAjNwS".contains(c) {
                let val = &a[2..];
                // re-dispatch as two tokens by rewriting the current token and re-running
                let mut v = args.to_vec();
                v[i] = format!("-{}", c);
                v.insert(i + 1, val.to_string());
                return run(&v, stdin, stdout, stderr);
            }
        }
        if a == "--" {
            no_more_opts = true;
            i += 1;
            continue;
        }
        if a == "--help" || a == "-h" {
            let _ = stdout.write_all(b"Usage: od [OPTION]... [FILE]\n");
            return 0;
        }
        if a == "--version" || a == "-V" {
            let _ = stdout.write_all(b"od (uutils coreutils) 0.10.0\n");
            return 0;
        }
        if a == "--traditional" {
            traditional = true;
            i += 1;
            continue;
        }
        if a == "--endian" && i + 1 < args.len() {
            endian = args[i + 1].as_str();
            i += 2;
            continue;
        }
        if a == "-A" || a == "--address-radix" {
            if i + 1 >= args.len() {
                let _ = stderr.write_all(b"od: missing operand\n");
                return 2;
            }
            radix = args[i + 1].chars().next().unwrap_or('o');
            i += 2;
            continue;
        }
        if a == "-j" || a == "--skip-bytes" {
            if i + 1 >= args.len() {
                let _ = stderr.write_all(b"od: missing operand\n");
                return 2;
            }
            skip = parse_bytes(&args[i + 1]);
            i += 2;
            continue;
        }
        if a == "-N" || a == "--read-bytes" {
            if i + 1 >= args.len() {
                let _ = stderr.write_all(b"od: missing operand\n");
                return 2;
            }
            read_bytes = parse_bytes(&args[i + 1]);
            i += 2;
            continue;
        }
        if a == "-w" || a == "--width" {
            if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                width = parse_bytes(&args[i + 1]).max(1) as usize;
                i += 2;
            } else {
                width = 32;
                i += 1;
            }
            continue;
        }
        if a == "-S" || a == "--strings" {
            if i + 1 < args.len() && !args[i + 1].starts_with('-') {
                strings_min = parse_bytes(&args[i + 1]) as usize;
                i += 2;
            } else {
                strings_min = 3;
                i += 1;
            }
            continue;
        }
        if a == "-v" || a == "--output-duplicates" {
            verbose = true;
            i += 1;
            continue;
        }
        if a == "-t" || a == "--format" {
            if i + 1 >= args.len() {
                let _ = stderr.write_all(b"od: missing operand\n");
                return 2;
            }
            for spec in args[i + 1].split(',') {
                let kind = spec.chars().next().unwrap_or('o');
                let rest: String = spec.chars().skip(1).filter(|c| *c != 'z').collect();
                formats.push(Fmt {
                    kind,
                    size: size_for(&rest, kind),
                    zdump: spec.ends_with('z'),
                });
            }
            i += 2;
            continue;
        }
        // single-letter format flags
        if a.len() == 2 && a.starts_with('-') {
            let c = a.chars().nth(1).unwrap();
            let kind = match c {
                'a' => 'a',
                'b' => 'o',
                'c' => 'c',
                'd' | 'D' | 'i' | 'l' | 's' => 'd',
                'o' | 'O' => 'o',
                'x' | 'h' | 'X' | 'H' => 'x',
                'e' | 'f' | 'F' => 'f',
                'u' => 'u',
                _ => 0 as char,
            };
            if kind != 0 as char {
                let size = match c {
                    'b' => 1,
                    'd' | 'o' | 'x' | 'h' | 's' => 2,
                    'D' | 'i' | 'O' | 'X' | 'H' | 'f' => 4,
                    'I' | 'L' | 'l' | 'e' | 'F' => 8,
                    'a' | 'c' => 1,
                    'u' => 4,
                    _ => 4,
                };
                formats.push(Fmt { kind, size, zdump: false });
                i += 1;
                continue;
            }
        }
        files.push(a.to_string());
        i += 1;
    }
    if formats.is_empty() && strings_min == 0 {
        formats.push(Fmt { kind: 'o', size: 2, zdump: false });
    }
    if files.is_empty() {
        files.push("-".to_string());
    }

    for file in &files {
        let data: Vec<u8> = if file == "-" {
            let mut buf = Vec::new();
            if stdin.read_to_end(&mut buf).is_err() {
                let _ = stderr.write_all(b"od: -: read error\n");
                return 1;
            }
            buf
        } else {
            match std::fs::File::open(file) {
                Ok(mut f) => {
                    let mut buf = Vec::new();
                    if f.read_to_end(&mut buf).is_err() {
                        let _ = stderr.write_all(&format!("od: {}: read error\n", file).into_bytes());
                        return 1;
                    }
                    buf
                }
                Err(e) => {
                    let _ = stderr.write_all(&format!("od: {}: {}\n", file, e).into_bytes());
                    return 1;
                }
            }
        };
        let mut data = data;
        if skip > 0 {
            let n = skip.min(data.len() as i64) as usize;
            data = data[n..].to_vec();
        }
        if read_bytes >= 0 && data.len() > read_bytes as usize {
            data.truncate(read_bytes as usize);
        }
        let big = endian == "big";
        let mut prev: Option<String> = None;
        let base = skip as usize;
        let mut off = base;
        while off - base < data.len() {
            let rel = off - base;
            let chunk_len = width.min(data.len() - rel);
            let chunk = &data[rel..rel + chunk_len];
            let addr = format_addr(off, radix);
            for (fi, f) in formats.iter().enumerate() {
                let mut line = String::new();
                if fi == 0 && radix != 'n' {
                    line.push_str(&addr);
                }
                let mut pos = 0;
                while pos < chunk.len() {
                    let sz = f.size.min(chunk.len() - pos);
                    let val = &chunk[pos..pos + sz];
                    let text = render_value(f, val, big);
                    line.push(' ');
                    let w = field_width(f);
                    if text.len() < w {
                        line.push_str(&" ".repeat(w - text.len()));
                    }
                    line.push_str(&text);
                    pos += sz;
                }
                if fi == 0 && !verbose {
                    let values = line[addr.len()..].to_string();
                    if prev.as_deref() == Some(values.as_str()) {
                        let _ = stdout.write_all(b"*\n");
                        continue;
                    }
                    prev = Some(values);
                }
                if f.zdump {
                    line.push(' ');
                    for &b in chunk {
                        line.push(if b.is_ascii_graphic() || b == b' ' {
                            b as char
                        } else {
                            '.'
                        });
                    }
                }
                if stdout.write_all(line.as_bytes()).is_err() {
                    return 1;
                }
                if stdout.write_all(b"\n").is_err() {
                    return 1;
                }
            }
            off += chunk_len;
        }
        if radix != 'n' && !traditional && !formats.is_empty() {
            let total = format_addr(base + data.len(), radix);
            if stdout.write_all(total.as_bytes()).is_err() {
                return 1;
            }
            if stdout.write_all(b"\n").is_err() {
                return 1;
            }
        }
        if strings_min > 0 {
            let mut run: Vec<u8> = Vec::new();
            for &b in &data {
                if b.is_ascii_graphic() {
                    run.push(b);
                } else {
                    if run.len() >= strings_min {
                        if stdout.write_all(&run).is_err() {
                            return 1;
                        }
                        if stdout.write_all(b"\n").is_err() {
                            return 1;
                        }
                    }
                    run.clear();
                }
            }
            if run.len() >= strings_min {
                if stdout.write_all(&run).is_err() {
                    return 1;
                }
                if stdout.write_all(b"\n").is_err() {
                    return 1;
                }
            }
        }
    }
    0
}

fn format_addr(off: usize, radix: char) -> String {
    match radix {
        'd' => format!("{:08}", off),
        'x' => format!("{:08x}", off),
        'n' => String::new(),
        _ => format!("{:07o}", off),
    }
}

fn field_width(f: &Fmt) -> usize {
    match f.kind {
        'o' => 3 * f.size,
        'x' => 2 * f.size,
        'c' | 'a' => 3,
        'f' => 6,
        _ => 2 * f.size + 1,
    }
}

fn render_value(f: &Fmt, val: &[u8], big: bool) -> String {
    match f.kind {
        'a' => {
            let b = val[0] & 0x7f;
            fmt_name(b)
        }
        'c' => {
            let b = val[0];
            match b {
                b'\n' => "\\n".to_string(),
                b'\t' => "\\t".to_string(),
                b'\r' => "\\r".to_string(),
                b'\\' => "\\\\".to_string(),
                0 => "\\0".to_string(),
                7 => "\\a".to_string(),
                8 => "\\b".to_string(),
                12 => "\\f".to_string(),
                11 => "\\v".to_string(),
                _ if b.is_ascii_graphic() || b == b' ' => (b as char).to_string(),
                _ => format!("\\{:03o}", b),
            }
        }
        'f' => {
            let v: f64 = if val.len() == 4 {
                let u = u32::from_be_bytes([val[0], val[1], val[2], val[3]]);
                if big {
                    f32::from_bits(u) as f64
                } else {
                    f32::from_le_bytes([val[0], val[1], val[2], val[3]]) as f64
                }
            } else if val.len() == 8 {
                let u = u64::from_be_bytes([val[0], val[1], val[2], val[3], val[4], val[5], val[6], val[7]]);
                if big {
                    f64::from_bits(u)
                } else {
                    f64::from_le_bytes([val[0], val[1], val[2], val[3], val[4], val[5], val[6], val[7]])
                }
            } else {
                0.0
            };
            let s = format!("{:.6}", v);
            s.trim_end_matches('0').trim_end_matches('.').to_string()
        }
        kind => {
            let mut v: u64 = 0;
            if big {
                for &b in val {
                    v = v * 256 + b as u64;
                }
            } else {
                for &b in val.iter().rev() {
                    v = v * 256 + b as u64;
                }
            }
            match kind {
                'o' => format!("{:0width$o}", v, width = 3 * f.size),
                'x' => format!("{:0width$x}", v, width = f.size * 2),
                'u' => format!("{:width$}", v, width = f.size * 2),
                'd' => {
                    let signed: i64 = match val.len() {
                        1 => (v as u8) as i8 as i64,
                        2 => (v as u16) as i16 as i64,
                        4 => (v as u32) as i32 as i64,
                        _ => v as i64,
                    };
                    format!("{:width$}", signed, width = f.size * 2 + 1)
                }
                _ => String::new(),
            }
        }
    }
}
