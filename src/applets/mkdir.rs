//! mkdir(1) — Create directories.
//!
//! 行為依本機參考實作（uutils coreutils 0.10.0）逐 case diff：
//! - 錯誤措辭 "mkdir: <op>: File exists"（無 "cannot create directory" 前綴）
//! - `-p`：已存在 rc0 靜默；每一層新建都印 -v 訊息
//! - `-m MODE`：octal 精確（不受 umask 影響）或 symbolic（GNU 語法）
//!   -p 時中間層用預設 mode、葉層用 -m
//! - `-Z/--context`：接受但 no-op（無 SELinux，見 assumptions）

use crate::{In, Out};
use std::fs;
use std::os::unix::fs::DirBuilderExt;

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut mode: Option<u32> = None;
    let mut parents = false;
    let mut verbose = false;
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
            match name {
                "parents" => parents = true,
                "verbose" => verbose = true,
                "help" => {
                    writeln!(stdout, "Usage: mkdir [OPTION]... DIRECTORY...\nCreate the DIRECTORY(ies), if they do not already exist.").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "mkdir (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                "mode" => {
                    let spec = if let Some(pos) = a.find('=') {
                        a[pos + 1..].to_string()
                    } else {
                        if i >= args.len() {
                            writeln!(stderr, "mkdir: option requires an argument -- 'mode'").ok();
                            return 2;
                        }
                        let v = args[i].clone();
                        i += 1;
                        v
                    };
                    match parse_mode(&spec, stderr) {
                        Some(m) => mode = Some(m),
                        None => return 1,
                    }
                }
                "context" => {
                    // -Z/--context[=CTX]：無 SELinux 環境下 no-op；僅在裸 --context
                    // 且下一參數不是選項時吞掉它
                    if !a.contains('=') && i < args.len() && !args[i].starts_with('-') {
                        i += 1;
                    }
                }
                _ => {
                    writeln!(stderr, "mkdir: unrecognized option '{}'", a).ok();
                    return 2;
                }
            }
            continue;
        }
        // short options
        let chars: Vec<char> = a[1..].chars().collect();
        let mut k = 0;
        while k < chars.len() {
            match chars[k] {
                'p' => parents = true,
                'v' => verbose = true,
                'Z' => {}
                'm' => {
                    let rest: String = chars[k + 1..].iter().collect();
                    let spec = if !rest.is_empty() {
                        rest
                    } else {
                        if i >= args.len() {
                            writeln!(stderr, "mkdir: option requires an argument -- 'm'").ok();
                            return 2;
                        }
                        let v = args[i].clone();
                        i += 1;
                        v
                    };
                    match parse_mode(&spec, stderr) {
                        Some(m) => mode = Some(m),
                        None => return 1,
                    }
                    break;
                }
                'h' => {
                    writeln!(stdout, "Usage: mkdir [OPTION]... DIRECTORY...").ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "mkdir (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => {
                    writeln!(stderr, "mkdir: invalid option -- '{}'", c).ok();
                    return 2;
                }
            }
            k += 1;
        }
    }
    if operands.is_empty() {
        writeln!(stderr, "mkdir: missing operand").ok();
        return 2;
    }
    let mut rc = 0;
    for op in &operands {
        if let Err(e) = mkdir_one(op, mode, parents, verbose, stdout) {
            if e.raw_os_error() == Some(17) {
                writeln!(stderr, "mkdir: {}: {}", op, crate::pathname::io_reason_nosuffix(&e)).ok();
            } else {
                writeln!(stderr, "mkdir: {}", crate::pathname::io_reason_nosuffix(&e)).ok();
            }
            rc = 1;
        }
    }
    rc
}

fn mkdir_one(op: &str, mode: Option<u32>, parents: bool, verbose: bool, stdout: Out) -> std::io::Result<()> {
    let mut create = |p: &std::path::Path, _leaf: bool| -> std::io::Result<()> {
        let mut db = fs::DirBuilder::new();
        db.mode(mode.unwrap_or(0o777));
        match db.create(p) {
            Ok(()) => {}
            Err(e) => {
                if e.raw_os_error() == Some(17 /* EEXIST */) {
                    if parents && p.is_dir() {
                        return Ok(());
                    }
                    if parents {
                        // -p：組件存在但不是目錄 → 真正原因是 ENOTDIR
                        return Err(std::io::Error::from_raw_os_error(20 /* ENOTDIR */));
                    }
                    return Err(e);
                }
                return Err(e);
            }
        }
        if verbose {
            writeln!(stdout, "mkdir: created directory '{}'", p.display()).ok();
        }
        Ok(())
    };
    let path = std::path::Path::new(op);
    if parents {
        let comp: Vec<_> = path.components().collect();
        let mut acc = std::path::PathBuf::new();
        for (idx, c) in comp.iter().enumerate() {
            if matches!(c, std::path::Component::RootDir) {
                acc.push("/");
                continue;
            }
            if matches!(c, std::path::Component::CurDir) {
                if acc.as_os_str().is_empty() {
                    acc.push(".");
                }
                continue;
            }
            acc.push(c.as_os_str());
            create(&acc, idx + 1 == comp.len())?;
        }
        if acc.as_os_str().is_empty() {
            return Ok(());
        }
    } else {
        create(path, true)?;
    }
    Ok(())
}

/// GNU mode 語法：octal（[0-7]+）或 symbolic clause 串
/// `[ugoa]*([-+=]([rwxXst]*|[ugo]))+`。
/// 錯位措辭與參考一致："invalid operator (expected +, -, or =, but found b)"。
pub fn parse_mode(spec: &str, stderr: Out) -> Option<u32> {
    let bytes: Vec<char> = spec.chars().collect();
    if bytes.is_empty() {
        writeln!(stderr, "mkdir: invalid mode '{}'", spec).ok();
        return None;
    }
    if bytes.iter().all(|c| c.is_ascii_digit()) {
        let v = u32::from_str_radix(spec, 8);
        if v.is_err() {
            writeln!(stderr, "mkdir: invalid mode '{}'", spec).ok();
        }
        return v.ok();
    }
    let mut perm = 0o777u32;
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] == ',' {
            i += 1;
            continue;
        }
        let mut who = 0u32;
        while i < bytes.len() && matches!(bytes[i], 'u' | 'g' | 'o' | 'a') {
            match bytes[i] {
                'u' => who |= 0o700,
                'g' => who |= 0o070,
                'o' => who |= 0o007,
                'a' => who |= 0o777,
                _ => {}
            }
            i += 1;
        }
        if who == 0 {
            who = 0o777;
        }
        if i >= bytes.len() {
            writeln!(stderr, "mkdir: invalid mode '{}'", spec).ok();
            return None;
        }
        let op = bytes[i];
        if !matches!(op, '+' | '-' | '=') {
            writeln!(stderr, "mkdir: invalid operator (expected +, -, or =, but found {})", op).ok();
            return None;
        }
        i += 1;
        let mut perms = 0u32;
        let mut perms_from_who: Vec<u32> = Vec::new();
        while i < bytes.len() && !matches!(bytes[i], '+' | '-' | '=' | ',') {
            match bytes[i] {
                'r' => perms |= 0o4,
                'w' => perms |= 0o2,
                'x' => perms |= 0o1,
                'X' => {}
                's' | 't' => {}
                'u' | 'g' | 'o' => {
                    perms_from_who.push(bytes[i] as u32);
                }
                _ => {
                    writeln!(stderr, "mkdir: invalid mode '{}'", spec).ok();
                    return None;
                }
            }
            i += 1;
        }
        for t in [6u32, 3, 0] {
            if (who >> t) & 0o7 == 0 {
                continue;
            }
            // 從 who 複製權限（u=g 型）：取對應欄位
            let mut p3 = perms;
            for w in &perms_from_who {
                let src = match *w {
                    c if c == 'u' as u32 => (perm >> 6) & 0o7,
                    c if c == 'g' as u32 => (perm >> 3) & 0o7,
                    _ => perm & 0o7,
                };
                p3 |= src;
            }
            match op {
                '+' => perm |= p3 << t,
                '-' => perm &= !(p3 << t),
                _ => perm = (perm & !(0o7 << t)) | (p3 << t),
            }
        }
    }
    Some(perm)
}
