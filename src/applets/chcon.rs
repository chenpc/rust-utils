//! chcon(1) — Change SELinux security context.
//!
//! 行為依本機參考實作（uutils coreutils 0.10.0）逐 case diff（docs/probes/cluster5.md）。
//! guest / host 皆無 SELinux 標籤 → 所有路徑失敗，措辭：
//! - 完整 CONTEXT：`chcon: Setting security context failed. setfilecon() failed on path 'P'. <reason (os error N)>.`
//!   （參考用 '/proc/self/fd/5' 內部 fd 路徑；我方直接用檔案路徑 → assumptions）
//! - partial（-u/-r/-l/-t 而無 CONTEXT）：`chcon: Applying partial security context to unlabeled file failed on 'FILE'. invalid input parameter.`
//! - `--reference=RFILE`：`chcon: Getting security context failed. getfilecon() failed on path 'RFILE'. <reason (os error N)>.`
//! - 無 FILE operand：`chcon: No files are specified.` + 空行 + Try 行

use crate::pathname::{io_reason_pub, usage_hint};
use crate::{In, Out};
use std::ffi::CString;

const XATTR: &str = "security.selinux";

extern "C" {
    fn lsetxattr(path: *const i8, name: *const i8, value: *const u8, size: usize, flags: i32) -> i32;
    fn lgetxattr(path: *const i8, name: *const i8, value: *mut u8, size: usize) -> isize;
}

fn errno_string() -> String {
    io_reason_pub(&std::io::Error::last_os_error())
}

fn set_context(path: &str, ctx: &[u8]) -> Result<(), String> {
    let c = match CString::new(path.as_bytes()) {
        Ok(c) => c,
        Err(_) => return Err("Invalid argument".to_string()),
    };
    let n = CString::new(XATTR).unwrap();
    let r = unsafe { lsetxattr(c.as_ptr(), n.as_ptr(), ctx.as_ptr(), ctx.len(), 0) };
    if r != 0 {
        return Err(errno_string());
    }
    Ok(())
}

fn get_context(path: &str) -> Result<Vec<u8>, String> {
    let c = match CString::new(path.as_bytes()) {
        Ok(c) => c,
        Err(_) => return Err("Invalid argument".to_string()),
    };
    let n = CString::new(XATTR).unwrap();
    let mut buf = vec![0u8; 1024];
    let r = unsafe { lgetxattr(c.as_ptr(), n.as_ptr(), buf.as_mut_ptr(), buf.len()) };
    if r < 0 {
        return Err(errno_string());
    }
    let len = (r as usize).min(buf.len());
    while len > 0 && buf[len - 1] == 0 {
        buf.truncate(len - 1);
    }
    Ok(buf)
}

struct Opts {
    recursive: bool,
    verbose: bool,
    reference: Option<String>,
    user: Option<String>,
    role: Option<String>,
    range: Option<String>,
    type_: Option<String>,
    operands: Vec<String>,
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    match run_inner(args, stdin, stdout, stderr) {
        Ok(code) => code,
        Err(code) => code,
    }
}

fn run_inner(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> Result<i32, i32> {
    let _ = stdout;
    let mut o = Opts {
        recursive: false,
        verbose: false,
        reference: None,
        user: None,
        role: None,
        range: None,
        type_: None,
        operands: Vec::new(),
    };
    let mut no_more_opts = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        i += 1;
        if no_more_opts || (!a.starts_with('-') || a == "-") {
            o.operands.push(a);
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            match name.as_str() {
                "reference" => {
                    match value_of(&a, args, &mut i, "reference", stderr) {
                        Ok(v) => o.reference = Some(v),
                        Err(code) => return Err(code),
                    }
                }
                "recursive" => o.recursive = true,
                "verbose" => o.verbose = true,
                "user" => match value_of(&a, args, &mut i, "user", stderr) {
                    Ok(v) => o.user = Some(v),
                    Err(code) => return Err(code),
                },
                "role" => match value_of(&a, args, &mut i, "role", stderr) {
                    Ok(v) => o.role = Some(v),
                    Err(code) => return Err(code),
                },
                "range" => match value_of(&a, args, &mut i, "range", stderr) {
                    Ok(v) => o.range = Some(v),
                    Err(code) => return Err(code),
                },
                "type" => match value_of(&a, args, &mut i, "type", stderr) {
                    Ok(v) => o.type_ = Some(v),
                    Err(code) => return Err(code),
                },
                "context" => {
                    if let Some(pos) = a.find('=') {
                        let v = a[pos + 1..].to_string();
                        if !v.is_empty() {
                            o.operands.insert(0, v);
                        }
                    }
                }
                "help" => {
                    writeln!(
                        stdout,
                        "Change the SELinux security context of each FILE to CONTEXT.\n\
                         With --reference, change the security context of each FILE to that of RFILE.\n\n\
                         Usage: chcon [OPTION]... CONTEXT FILE...\n\
                         chcon [OPTION]... [-u USER] [-r ROLE] [-l RANGE] [-t TYPE] FILE...\n\
                         chcon [OPTION]... --reference=RFILE FILE..."
                    )
                    .ok();
                    return Err(0);
                }
                "version" => {
                    writeln!(stdout, "chcon (uutils coreutils) 0.10.0").ok();
                    return Err(0);
                }
                _ => {
                    writeln!(stderr, "error: unexpected argument '{}' found", a).ok();
                    writeln!(stderr).ok();
                    writeln!(stderr, "For more information, try '--help'.").ok();
                    return Err(1);
                }
            }
            continue;
        }
        let chars: Vec<char> = a[1..].chars().collect();
        let mut k = 0usize;
        while k < chars.len() {
            let c = chars[k];
            match c {
                'R' => o.recursive = true,
                'v' => o.verbose = true,
                'h' => {
                    writeln!(
                        stdout,
                        "Change the SELinux security context of each FILE to CONTEXT.\n\
                         With --reference, change the security context of each FILE to that of RFILE.\n\n\
                         Usage: chcon [OPTION]... CONTEXT FILE...\n\
                         chcon [OPTION]... [-u USER] [-r ROLE] [-l RANGE] [-t TYPE] FILE...\n\
                         chcon [OPTION]... --reference=RFILE FILE..."
                    )
                    .ok();
                    return Err(0);
                }
                'V' => {
                    writeln!(stdout, "chcon (uutils coreutils) 0.10.0").ok();
                    return Err(0);
                }
                'u' | 'r' | 'l' | 't' => {
                    let rest: String = chars[k + 1..].iter().collect();
                    let v = if !rest.is_empty() {
                        rest
                    } else {
                        if i >= args.len() {
                            crate::pathname::clap_value_required(&c.to_string(), "CONTEXT", stderr);
                            return Err(2);
                        }
                        let v = args[i].clone();
                        i += 1;
                        v
                    };
                    k = chars.len();
                    match c {
                        'u' => o.user = Some(v),
                        'r' => o.role = Some(v),
                        'l' => o.range = Some(v),
                        _ => o.type_ = Some(v),
                    }
                }
                'Z' => {
                    let rest: String = chars[k + 1..].iter().collect();
                    if !rest.is_empty() {
                        o.operands.insert(0, rest);
                        k = chars.len();
                    } else if i < args.len() && !args[i].starts_with('-') {
                        let v = args[i].clone();
                        i += 1;
                        o.operands.insert(0, v);
                    }
                }
                c => {
                    writeln!(stderr, "error: unexpected argument '-{}' found", c).ok();
                    writeln!(stderr).ok();
                    writeln!(stderr, "For more information, try '--help'.").ok();
                    return Err(1);
                }
            }
            k += 1;
        }
    }

    // 最後一個非選項 operand 之前的是 CONTEXT（位置型）
    let files: Vec<String>;
    let context: Option<String>;
    if o.reference.is_some() {
        files = o.operands.clone();
        context = None;
    } else if !o.operands.is_empty() {
        context = Some(o.operands[0].clone());
        files = o.operands[1..].to_vec();
    } else {
        files = Vec::new();
        context = None;
    }

    if files.is_empty() {
        writeln!(stderr, "chcon: No files are specified.").ok();
        writeln!(stderr).ok();
        writeln!(stderr, "{}", usage_hint("chcon")).ok();
        return Err(1);
    }

    let partial = context.is_none()
        && (o.user.is_some() || o.role.is_some() || o.range.is_some() || o.type_.is_some());

    let mut rc = 0;
    for f in &files {
        for path in walk(f, o.recursive) {
            if let Some(rfile) = &o.reference {
                match get_context(rfile) {
                    Ok(ctx) => {
                        if let Err(e) = set_context(&path, &ctx) {
                            writeln!(
                                stderr,
                                "chcon: Setting security context failed. setfilecon() failed on path '{}'. {}.",
                                path, e
                            )
                            .ok();
                            rc = 1;
                        } else if o.verbose {
                            writeln!(stderr, "chcon: setting context of '{}'", path).ok();
                        }
                    }
                    Err(e) => {
                        writeln!(
                            stderr,
                            "chcon: Getting security context failed. getfilecon() failed on path '{}'. {}.",
                            rfile, e
                        )
                        .ok();
                        rc = 1;
                    }
                }
                continue;
            }
            if partial {
                writeln!(
                    stderr,
                    "chcon: Applying partial security context to unlabeled file failed on '{}'. invalid input parameter.",
                    path
                )
                .ok();
                rc = 1;
                continue;
            }
            let ctx = context.clone().unwrap_or_default();
            let mut bytes = ctx.as_bytes().to_vec();
            bytes.push(0);
            match set_context(&path, &bytes) {
                Ok(()) => {
                    if o.verbose {
                        writeln!(stderr, "chcon: setting context of '{}'", path).ok();
                    }
                }
                Err(e) => {
                    writeln!(
                        stderr,
                        "chcon: Setting security context failed. setfilecon() failed on path '{}'. {}.",
                        path, e
                    )
                    .ok();
                    rc = 1;
                }
            }
        }
    }
    Ok(rc)}

fn value_of(
    a: &str,
    args: &[String],
    i: &mut usize,
    long: &str,
    stderr: Out,
) -> Result<String, i32> {
    if let Some(pos) = a.find('=') {
        return Ok(a[pos + 1..].to_string());
    }
    if *i >= args.len() {
        crate::pathname::clap_value_required(long, "VALUE", stderr);
        return Err(2);
    }
    let v = args[*i].clone();
    *i += 1;
    Ok(v)
}

/// -R 時展開目錄（uutils 順序：目錄本身先，再子項）。
fn walk(path: &str, recursive: bool) -> Vec<String> {
    let mut out = vec![path.to_string()];
    if !recursive {
        return out;
    }
    if let Ok(md) = std::fs::symlink_metadata(path) {
        if !md.file_type().is_dir() {
            return out;
        }
        if let Ok(rd) = std::fs::read_dir(path) {
            let mut entries: Vec<String> = rd
                .filter_map(|e| e.ok())
                .map(|e| e.path().to_string_lossy().to_string())
                .collect();
            entries.sort();
            for e in entries {
                out.extend(walk(&e, true));
            }
        }
    }
    out
}
