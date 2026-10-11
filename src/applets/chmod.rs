// chmod：權限變更。措辭對齊參考 uutils coreutils 0.10.0。
//
// - 錯誤："chmod: cannot access 'X': <reason>"（stat 失敗）；
//   "chmod: invalid operator (expected +, -, or =, but found b)"（symbolic 語法）
// - -c："mode of 'X' changed from 0644 (rw-r--r--) to 0750 (rwxr-x---)"
// - -v：未變更時 "mode of 'X' retained as 0750 (...)"
// - -f：抑制錯誤訊息但 **rc 仍為 1**（參考行為；GNU 為 rc0 → 記錄於 assumptions）
// - --from 參考不支援（clap 拒絕 rc1）→ 不實作
// - -h：fchmodat(AT_SYMLINK_NOFOLLOW)
// - symbolic `a+x` 的 umask 語意、u/g 複製權限不實作（assumptions）

use std::fs;
use std::io::Write;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

pub fn current_umask() -> u32 {
    extern "C" {
        fn umask(mask: u32) -> u32;
    }
    let m = unsafe { umask(0) };
    unsafe { umask(m) };
    m
}

pub enum Op {
    Octal(u32),
    Sym(u32, char, u32),
}

pub struct Spec(pub Vec<Op>);

pub fn run(args: &[String], _stdin: crate::In, stdout: crate::Out, stderr: crate::Out) -> i32 {
    let mut changes = false;
    let mut silent = false;
    let mut no_deref = false;
    let mut recursive = false;
    let mut verbose = false;
    let mut reference: Option<String> = None;
    let mut operands: Vec<String> = Vec::new();
    let mut mode_spec: Option<String> = None;
    let mut i = 0usize;
    let mut only_operands = false;
    while i < args.len() {
        let a = args[i].as_str();
        if only_operands {
            operands.push(a.to_string());
            i += 1;
            continue;
        }
        if a == "--" {
            only_operands = true;
            i += 1;
            continue;
        }
        if let Some(long) = a.strip_prefix("--") {
            let (name, val) = match long.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None => (long, None),
            };
            let getval = |i: &mut usize| -> Option<String> {
                if let Some(v) = val.clone() {
                    return Some(v);
                }
                *i += 1;
                args.get(*i).cloned()
            };
            match name {
                "changes" => changes = true,
                "silent" | "quiet" => silent = true,
                "no-dereference" => no_deref = true,
                "reference" => match getval(&mut i) {
                    Some(v) => reference = Some(v),
                    None => {
                        writeln!(stderr, "chmod: option requires an argument -- 'reference'").ok();
                        return 2;
                    }
                },
                "recursive" => recursive = true,
                "verbose" => verbose = true,
                "help" => {
                    writeln!(stdout, "Usage: chmod [OPTION]... MODE[,MODE]... FILE...\nAdjust the permissions of each FILE").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "chmod (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => {
                    writeln!(stderr, "chmod: unrecognized option '{}'", a).ok();
                    return 2;
                }
            }
            i += 1;
            continue;
        }
        if a.len() > 1 && a.starts_with('-') {
            let chars: Vec<char> = a[1..].chars().collect();
            let mut k = 0usize;
            while k < chars.len() {
                match chars[k] {
                    'c' => changes = true,
                    'f' => silent = true,
                    'h' => no_deref = true,
                    'R' => recursive = true,
                    'v' => verbose = true,
                    c => {
                        writeln!(stderr, "chmod: invalid option -- '{}'", c).ok();
                        return 2;
                    }
                }
                k += 1;
            }
            i += 1;
            continue;
        }
        // GNU：第一個非選項參數是 MODE，其餘是 FILE
        if mode_spec.is_none() && reference.is_none() {
            mode_spec = Some(a.to_string());
        } else {
            operands.push(a.to_string());
        }
        i += 1;
    }

    if operands.is_empty() || (mode_spec.is_none() && reference.is_none()) {
        writeln!(stderr, "chmod: missing operand").ok();
        return 2;
    }

    let spec = match &mode_spec {
        Some(m) => match parse_mode_chmod(m) {
            Ok(s) => Some(s),
            Err(e) => {
                if !silent {
                    writeln!(stderr, "chmod: {}", e).ok();
                }
                return 1;
            }
        },
        None => None,
    };

    let mut rc = 0;
    for op in &operands {
        rc = process(op, spec.as_ref(), reference.as_deref(), changes, verbose, silent, no_deref, recursive, stderr, 0);
    }
    rc
}

#[allow(clippy::too_many_arguments)]
fn process(
    path: &str,
    spec: Option<&Spec>,
    reference: Option<&str>,
    changes: bool,
    verbose: bool,
    silent: bool,
    no_deref: bool,
    recursive: bool,
    stderr: &mut dyn Write,
    mut rc: i32,
) -> i32 {
    let meta = if no_deref {
        fs::symlink_metadata(path)
    } else {
        fs::metadata(path)
    };
    let meta = match meta {
        Ok(m) => m,
        Err(e) => {
            if !silent {
                writeln!(stderr, "chmod: cannot access '{}': {}", path, crate::pathname::io_reason_pub(&e)).ok();
            }
            return 1;
        }
    };
    let is_dir = meta.is_dir();
    let cur = meta.mode() & 0o7777;
    let target = if let Some(rf) = reference {
        match fs::metadata(rf) {
            Ok(rm) => rm.mode() & 0o7777,
            Err(e) => {
                if !silent {
                    writeln!(stderr, "chmod: failed to get attributes of '{}': {}", rf, crate::pathname::io_reason_pub(&e)).ok();
                }
                return 1;
            }
        }
    } else {
        apply_spec(cur, spec.unwrap())
    };
    let changed = cur != target;
    let res = if no_deref {
        lchmod(path, target)
    } else {
        fs::set_permissions(path, fs::Permissions::from_mode(target))
    };
    match res {
        Ok(()) => {
            if verbose {
                if changed {
                    writeln!(stderr, "mode of '{}' changed from {} to {}", path, fmt_mode(cur), fmt_mode(target)).ok();
                } else {
                    writeln!(stderr, "mode of '{}' retained as {}", path, fmt_mode(target)).ok();
                }
            } else if changes && changed {
                writeln!(stderr, "mode of '{}' changed from {} to {}", path, fmt_mode(cur), fmt_mode(target)).ok();
            }
        }
        Err(e) => {
            if !silent {
                writeln!(stderr, "chmod: changing permissions of '{}': {}", path, crate::pathname::io_reason_pub(&e)).ok();
            }
            rc = 1;
        }
    }
    if recursive && is_dir {
        if let Ok(rd) = fs::read_dir(path) {
            let mut entries: Vec<String> = rd.flatten().map(|e| e.path().to_string_lossy().into_owned()).collect();
            entries.sort();
            for p in entries {
                rc = process(&p, spec, None, changes, verbose, silent, no_deref, true, stderr, rc);
            }
        }
    }
    rc
}

pub fn apply_spec(cur: u32, spec: &Spec) -> u32 {
    let mut acc = cur;
    for op in &spec.0 {
        acc = match op {
            Op::Octal(v) => *v,
            Op::Sym(who, o, perms) => match o {
                '=' => (acc & !who) | *perms,
                '+' => acc | *perms,
                _ => acc & !*perms,
            },
        };
    }
    acc & 0o7777
}

fn lchmod(path: &str, mode: u32) -> std::io::Result<()> {
    use std::ffi::CString;
    extern "C" {
        fn fchmodat(dirfd: i32, path: *const i8, mode: u32, flags: i32) -> i32;
    }
    let c = CString::new(path).map_err(|_| std::io::Error::from_raw_os_error(22))?;
    // AT_FDCWD=-100, AT_SYMLINK_NOFOLLOW=0x100
    if unsafe { fchmodat(-100, c.as_ptr(), mode, 0x100) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

pub fn fmt_mode(m: u32) -> String {
    let m = m & 0o7777;
    format!("{:04o} ({})", m, tri_letters(m))
}

fn tri_letters(m: u32) -> String {
    let mut out = String::new();
    let triples = [(0o400u32, 0o200u32, 0o100u32), (0o040, 0o020, 0o010), (0o004, 0o002, 0o001)];
    let specials = [(0o4000u32, 0o100u32, 's'), (0o2000u32, 0o010u32, 's'), (0o1000u32, 0o001u32, 't')];
    for (i, (r, w, _x)) in triples.iter().enumerate() {
        out.push(if m & r != 0 { 'r' } else { '-' });
        out.push(if m & w != 0 { 'w' } else { '-' });
        let (sbit, xbit, letter) = specials[i];
        if m & sbit != 0 {
            out.push(if m & xbit != 0 { letter } else { letter.to_ascii_uppercase() });
        } else {
            out.push(if m & xbit != 0 { 'x' } else { '-' });
        }
    }
    out
}

/// 解析 "MODE[,MODE]"：逐段依序套用（octal 為絕對值）。
pub fn parse_mode_chmod(spec: &str) -> Result<Spec, String> {
    let mut ops: Vec<Op> = Vec::new();
    for part in spec.split(',') {
        let bytes: Vec<char> = part.chars().collect();
        if bytes.is_empty() {
            return Err(format!("invalid mode '{}'", spec));
        }
        if bytes[0].is_ascii_digit() {
            let v = u32::from_str_radix(part, 8).map_err(|_| "invalid digit found in string".to_string())?;
            if v > 0o7777 {
                return Err(format!("invalid mode '{}'", spec));
            }
            ops.push(Op::Octal(v));
            continue;
        }
        let mut idx = 0usize;
        let mut cur_who: Option<u32> = None;
        while idx < bytes.len() {
            let mut who: u32 = 0;
            while idx < bytes.len() && matches!(bytes[idx], 'a' | 'u' | 'g' | 'o') {
                match bytes[idx] {
                    'a' => who |= 0o7777,
                    'u' => who |= 0o0700,
                    'g' => who |= 0o0070,
                    _ => who |= 0o0007,
                }
                idx += 1;
            }
            if who == 0 {
                who = match cur_who {
                    Some(w) => w,
                    None => 0o777 & !current_umask(),
                };
            }
            cur_who = Some(who);
            if idx >= bytes.len() {
                return Err(format!("invalid mode ({})", part));
            }
            if !matches!(bytes[idx], '+' | '-' | '=') {
                let c = bytes[idx];
                return Err(format!("invalid operator (expected +, -, or =, but found {})", c));
            }
            let op = bytes[idx];
            idx += 1;
            let mut base: u32 = 0;
            let mut sp: u32 = 0;
            while idx < bytes.len() && matches!(bytes[idx], 'r' | 'w' | 'x' | 's' | 't') {
                match bytes[idx] {
                    'r' => base |= 0o444,
                    'w' => base |= 0o222,
                    'x' => base |= 0o111,
                    's' => {
                        if who & 0o0700 != 0 {
                            sp |= 0o4000;
                        }
                        if who & 0o0070 != 0 {
                            sp |= 0o2000;
                        }
                    }
                    _ => {
                        if who & 0o0007 != 0 {
                            sp |= 0o1000;
                        }
                    }
                }
                idx += 1;
            }
            ops.push(Op::Sym(who, op, (base & who) | sp));
        }
    }
    if ops.is_empty() {
        return Err(format!("invalid mode '{}'", spec));
    }
    Ok(Spec(ops))
}
