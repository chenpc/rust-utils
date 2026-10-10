//! truncate(1) — Shrink or extend the size of each FILE to SIZE.
//!
//! 措辭與能力依參考（uutils 0.10.0）逐 case 探測：
//! - `-s SIZE`：前綴 `+ - < >` 支援；units K/M/…(1024)、KB/MB(1000)；`0x` hex 支援
//! - **`/` 與 `%`（round to multiple）參考不支援**："truncate: Invalid number: '20%'" rc1
//! - `-r FILE`："truncate: cannot stat 'X': No such file or directory" rc1
//! - 不存在的 operand 會建立；`/` → "truncate: cannot open '/' for writing: Is a directory"
//! - "R" → "truncate: Invalid number: 'R': Value too large for defined data type"（參考把 R
//!   當 0 大小嘗試；我方對無效數字統一措辭，見 assumptions）

use crate::{In, Out};
use std::fs::OpenOptions;

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut size_spec: Option<String> = None;
    let mut ref_file: Option<String> = None;
    let mut operands: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if no_more_opts || (!a.starts_with('-') || a == "-") {
            operands.push(a.clone());
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("");
            let getval = |a: &String, i: &mut usize| -> Option<String> {
                if let Some(pos) = a.find('=') {
                    Some(a[pos + 1..].to_string())
                } else if *i < args.len() {
                    let v = args[*i].clone();
                    *i += 1;
                    Some(v)
                } else {
                    None
                }
            };
            match name {
                "size" => match getval(a, &mut i) {
                    Some(v) => size_spec = Some(v),
                    None => {
                        writeln!(stderr, "truncate: option requires an argument -- 'size'").ok();
                        return 2;
                    }
                },
                "reference" => match getval(a, &mut i) {
                    Some(v) => ref_file = Some(v),
                    None => {
                        writeln!(stderr, "truncate: option requires an argument -- 'reference'").ok();
                        return 2;
                    }
                },
                "help" => {
                    writeln!(stdout, "Usage: truncate [OPTION]... FILE...\nAdjust the size of each FILE to the specified SIZE").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "truncate (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => {
                    writeln!(stderr, "truncate: unrecognized option '{}'", a).ok();
                    return 2;
                }
            }
            continue;
        }
        let chars: Vec<char> = a[1..].chars().collect();
        let mut k = 0;
        while k < chars.len() {
            match chars[k] {
                's' | 'r' => {
                    let opt = chars[k];
                    let rest: String = chars[k + 1..].iter().collect();
                    let v = if !rest.is_empty() {
                        rest
                    } else if i < args.len() {
                        let v = args[i].clone();
                        i += 1;
                        v
                    } else {
                        writeln!(stderr, "truncate: option requires an argument -- '{}'", opt).ok();
                        return 2;
                    };
                    if opt == 's' {
                        size_spec = Some(v);
                    } else {
                        ref_file = Some(v);
                    }
                    break;
                }
                'c' => {} // GNU 相容：忽略
                'h' => {
                    writeln!(stdout, "Usage: truncate [OPTION]... FILE...").ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "truncate (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => {
                    writeln!(stderr, "truncate: invalid option -- '{}'", c).ok();
                    return 2;
                }
            }
            k += 1;
        }
    }
    if operands.is_empty() {
        writeln!(stderr, "truncate: missing operand").ok();
        return 2;
    }
    if size_spec.is_none() && ref_file.is_none() {
        writeln!(stderr, "truncate: you must specify one of -s and --reference").ok();
        return 1;
    }
    // -r 優先取 reference size；-s 套用於其上（GNU：兩者皆給時先 ref 再 arithmetic）
    let ref_size: Option<u64> = if let Some(rf) = &ref_file {
        match std::fs::metadata(rf) {
            Ok(m) => Some(m.len()),
            Err(e) => {
                writeln!(stderr, "truncate: cannot stat '{}': {}", rf, crate::pathname::io_reason_nosuffix(&e)).ok();
                return 1;
            }
        }
    } else {
        None
    };
    let mut rc = 0;
    for f in &operands {
        let file = match OpenOptions::new().write(true).create(true).open(f) {
            Ok(f) => f,
            Err(e) => {
                writeln!(stderr, "truncate: cannot open '{}' for writing: {}", f, crate::pathname::io_reason_nosuffix(&e)).ok();
                rc = 1;
                continue;
            }
        };
        // -r 提供基準大小；-s 的算術前綴套用於其上（GNU 語意）
        let cur = ref_size.unwrap_or_else(|| file.metadata().map(|m| m.len()).unwrap_or(0));
        let newsize: u64 = if let Some(spec) = &size_spec {
            let mut spec = spec.as_str();
            let mut op = '=';
            if let Some(c) = spec.chars().next() {
                if matches!(c, '+' | '-' | '<' | '>') {
                    op = c;
                    spec = &spec[1..];
                }
            }
            match parse_size(spec) {
                Some(n) => match op {
                    '=' => n,
                    '+' => cur.saturating_add(n),
                    '-' => cur.saturating_sub(n),
                    '<' => cur.min(n),
                    _ => cur.max(n),
                },
                None => {
                    writeln!(stderr, "truncate: Invalid number: '{}'", size_spec.as_deref().unwrap_or("")).ok();
                    rc = 1;
                    continue;
                }
            }
        } else {
            cur
        };
        if let Err(e) = file.set_len(newsize) {
            writeln!(stderr, "truncate: cannot truncate '{}': {}", f, crate::pathname::io_reason_nosuffix(&e)).ok();
            rc = 1;
        }
    }
    rc
}

/// SIZE：整數（可 0x hex）+ 可選單位 K M G T P E Z Y（1024）/ KB MB …（1000）。
fn parse_size(s: &str) -> Option<u64> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let mut idx = 0usize;
    let b = s.as_bytes();
    if b.starts_with(b"0x") || b.starts_with(b"0X") {
        idx = 2;
        let rest = &s[idx..];
        let num: String = rest.chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        if num.is_empty() || num.len() != rest.len() {
            return None;
        }
        return u64::from_str_radix(&num, 16).ok();
    }
    let num: String = s[idx..].chars().take_while(|c| c.is_ascii_digit()).collect();
    if num.is_empty() {
        return None;
    }
    idx += num.len();
    let mut n: u64 = num.parse().ok()?;
    let unit = &s[idx..];
    let mult: u64 = match unit {
        "" => 1,
        "K" | "k" | "KiB" => 1024,
        "M" | "MiB" => 1024 * 1024,
        "G" | "GiB" => 1024u64.pow(3),
        "T" | "TiB" => 1024u64.pow(4),
        "P" | "PiB" => 1024u64.pow(5),
        "E" | "EiB" => 1024u64.pow(6),
        "Z" | "ZiB" => 1024u64.pow(7),
        "Y" | "YiB" => 1024u64.pow(8),
        "KB" | "kB" => 1000,
        "MB" | "mB" => 1000 * 1000,
        "GB" | "gB" => 1000u64.pow(3),
        "TB" | "tB" => 1000u64.pow(4),
        "PB" | "pB" => 1000u64.pow(5),
        "EB" | "eB" => 1000u64.pow(6),
        "ZB" | "zB" => 1000u64.pow(7),
        "YB" | "yB" => 1000u64.pow(8),
        "B" => 1,
        _ => return None,
    };
    n = n.checked_mul(mult)?;
    Some(n)
}
