//! tr(1) — Translate or delete characters.
//!
//! 依據 man page（docs/man/tr.1.gz）：
//! - `-c|--complement` 用 SET1 的補集
//! - `-d|--delete` 刪除 SET1 中的字元，不翻譯
//! - `-s|--squeeze-repeats` 把「最後指定的 SET」中重複出現的連續字元壓成單一
//! - `-t|--truncate-set1` 先把 SET1 截到 SET2 的長度
//! - `-h|--help`、`-V|--version`
//!
//! man page 未定義：SET1 比 SET2 長時的對應方式、字元集/範圍語法、class 名稱。
//! 假設（docs/assumptions.md）：支援 `a-z` 範圍、`[:class:]`、`\` escape 與八位 `\NNN`；
//! SET1 比 SET2 長時用 SET2 的最後一個字元補；處理順序 = 翻譯（或刪除）後才 squeeze。

use crate::{In, Out};

fn expand_set(s: &str) -> Vec<u8> {
    let b = s.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'[' && i + 1 < b.len() && b[i + 1] == b':' {
            let end = i + 2;
            match class_range(s, end) {
                Some((chars, next)) => {
                    out.extend_from_slice(&chars);
                    i = next;
                    continue;
                }
                None => {}
            }
        }
        let c = decode_char(b, &mut i);
        if i + 1 < b.len() && b[i] == b'-' {
            i += 1;
            let d = decode_char(b, &mut i);
            let (lo, hi) = if c <= d { (c, d) } else { (d, c) };
            for x in lo..=hi {
                out.push(x);
            }
        } else {
            out.push(c);
        }
    }
    out
}

fn decode_char(b: &[u8], i: &mut usize) -> u8 {
    if b[*i] == b'\\' && *i + 1 < b.len() {
        *i += 1;
        let c = b[*i];
        let mapped = match c {
            b'\\' => b'\\',
            b'a' => 0x07,
            b'b' => 0x08,
            b'f' => 0x0C,
            b'n' => b'\n',
            b'r' => b'\r',
            b't' => b'\t',
            b'v' => 0x0B,
            b'0'..=b'7' => {
                let mut v = (c - b'0') as u32;
                while *i + 1 < b.len() && b[*i + 1] >= b'0' && b[*i + 1] <= b'7' {
                    *i += 1;
                    v = v * 8 + (b[*i] - b'0') as u32;
                }
                *i += 1;
                return (v & 0xFF) as u8;
            }
            other => other,
        };
        *i += 1;
        return mapped;
    }
    let c = b[*i];
    *i += 1;
    c
}

fn class_range(s: &str, start: usize) -> Option<(Vec<u8>, usize)> {
    // 語法：[:name:] —— name 之後必須是 ':' 再 ']'，所以不能直接 find(']')
    let rel = s[start..].find(':')?;
    let end = start + rel;
    let name = &s[start..end];
    if end + 1 >= s.len() || s.as_bytes()[end + 1] != b']' {
        return None;
    }
    let chars: Vec<u8> = match name {
        "lower" => (b'a'..=b'z').collect(),
        "upper" => (b'A'..=b'Z').collect(),
        "digit" => (b'0'..=b'9').collect(),
        "alnum" => (b'a'..=b'z').chain(b'A'..=b'Z').chain(b'0'..=b'9').collect(),
        "space" => vec![b' ', b'\t', b'\n', 0x0B, 0x0C, b'\r'],
        "blank" => vec![b' ', b'\t'],
        "punct" => (0x21u8..=0x2F)
            .chain(0x3A..=0x40)
            .chain(0x5B..=0x60)
            .chain(0x7B..=0x7E)
            .collect(),
        "graph" => (0x21u8..=0x7E).collect(),
        "print" => (0x20u8..=0x7E).collect(),
        "cntrl" => (0x00u8..=0x1F).chain(0x7F..=0x7F).collect(),
        _ => return None,
    };
    Some((chars, end + 2))
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut complement = false;
    let mut delete = false;
    let mut squeeze = false;
    let mut truncate = false;
    let mut sets: Vec<String> = Vec::new();

    for a in args {
        match a.as_str() {
            "-h" | "--help" => {
                writeln!(stdout, "Usage: tr [-c] [-d] [-s] [-t] [-h|--help] [-V|--version] [sets]").ok();
                return 0;
            }
            "-V" | "--version" => {
                writeln!(stdout, "tr (rust-coreutils) 0.1.0").ok();
                return 0;
            }
            s if s.starts_with('-') && s.len() > 1 && s != "--" => {
                for ch in s[1..].chars() {
                    match ch {
                        'c' => complement = true,
                        'd' => delete = true,
                        's' => squeeze = true,
                        't' => truncate = true,
                        _ => {}
                    }
                }
            }
            other => sets.push(other.to_string()),
        }
    }

    if sets.is_empty() {
        writeln!(stderr, "tr: missing operand").ok();
        return 2;
    }

    let mut set1 = expand_set(&sets[0]);
    if complement {
        let mut comp: Vec<u8> = Vec::new();
        for x in 0u8..=255 {
            if !set1.contains(&x) {
                comp.push(x);
            }
        }
        set1 = comp;
    }
    let set2 = if sets.len() > 1 { expand_set(&sets[1]) } else { Vec::new() };
    if truncate && !set2.is_empty() {
        set1.truncate(set2.len());
    }
    let squeeze_set = expand_set(&sets[sets.len() - 1]);

    let mut input = Vec::new();
    if stdin.read_to_end(&mut input).is_err() {
        writeln!(stderr, "tr: read error").ok();
        return 1;
    }

    let mut prev: Option<u8> = None;
    for &b in &input {
        let idx = set1.iter().position(|&x| x == b);
        let mut out_byte = b;
        if delete {
            if idx.is_some() {
                continue;
            }
        } else if !set2.is_empty() {
            if let Some(k) = idx {
                out_byte = set2[k.min(set2.len() - 1)];
            }
        }
        if squeeze {
            if squeeze_set.contains(&out_byte) && prev == Some(out_byte) {
                continue;
            }
        }
        prev = Some(out_byte);
        if stdout.write_all(&[out_byte]).is_err() {
            return 1;
        }
    }
    stdout.flush().ok();
    0
}
