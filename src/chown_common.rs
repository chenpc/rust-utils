// chown/chgrp 共用核心（非 applet 模組）。措辭對齊參考 uutils coreutils 0.10.0。
//
// - 名稱→id 解析讀 /etc/passwd、/etc/group；找不到："chown: invalid user: 'x'"
// - stat 失敗："chown: cannot access 'X': <reason>"
// - symlink 未加 -h 且 dangling："chown: cannot dereference 'X': <reason>"
// - syscall 失敗（非遞迴）："chown: changing ownership of 'X': <reason (os error N)>"
//   （遞迴路徑的錯誤 **不含** " (os error N)" 後綴 — 參考 quirk，記錄 assumptions）
// - -c/-v 成功："changed ownership of 'X' from a:b to c:d"；-v 失敗："failed to change ownership of 'X' from a:b to c:d"
// - 顯示名稱查 passwd/group，找不到退回數字
// - --from 參考不支援 → 不實作

use std::fs;
use std::io::Write;
use std::os::unix::fs::MetadataExt;

pub struct ChOpts {
    pub traverse_symlinks: bool, // 預設 true（跟隨）
    pub recursive: bool,
    pub changes: bool,
    pub verbose: bool,
    pub silent: bool,
    pub reference: Option<String>,
}

/// 回傳 (new_uid, new_gid)；None 欄表示不變更。
pub fn parse_owner(
    spec: &str,
    want_user: bool,
    want_group_only: bool,
    prog: &str,
    stderr: &mut dyn Write,
) -> Option<(Option<u32>, Option<u32>)> {
    let (uspec, gspec) = match spec.find([':', '.']) {
        Some(i) => (&spec[..i], Some(&spec[i + 1..])),
        None => (spec, None),
    };
    let mut uid = None;
    let mut gid = None;
    if want_group_only {
        if !uspec.is_empty() {
            gid = Some(match parse_id(uspec, true) {
                Some(v) => v,
                None => {
                    writeln!(stderr, "{}: invalid group: '{}'", prog, uspec).ok();
                    return None;
                }
            });
        }
        if let Some(g) = gspec {
            if !g.is_empty() {
                gid = Some(match parse_id(g, true) {
                    Some(v) => v,
                    None => {
                        writeln!(stderr, "{}: invalid group: '{}'", prog, g).ok();
                        return None;
                    }
                });
            }
        }
        return Some((None, gid));
    }
    if !uspec.is_empty() {
        uid = Some(match parse_id(uspec, false) {
            Some(v) => v,
            None => {
                writeln!(stderr, "{}: invalid user: '{}'", prog, uspec).ok();
                return None;
            }
        });
    }
    if let Some(g) = gspec {
        if !g.is_empty() {
            gid = Some(match parse_id(g, true) {
                Some(v) => v,
                None => {
                    writeln!(stderr, "{}: invalid group: '{}'", prog, g).ok();
                    return None;
                }
            });
        }
    }
    let _ = want_user;
    Some((uid, gid))
}

fn parse_id(s: &str, group: bool) -> Option<u32> {
    if let Ok(v) = s.parse::<u32>() {
        return Some(v);
    }
    lookup_name(s, group)
}

/// 從 /etc/passwd（name:pass:uid:...）或 /etc/group（name:pass:gid:...）查名稱。
pub fn lookup_name(name: &str, group: bool) -> Option<u32> {
    let path = if group { "/etc/group" } else { "/etc/passwd" };
    let data = fs::read_to_string(path).ok()?;
    for line in data.lines() {
        let f: Vec<&str> = line.split(':').collect();
        if f.len() >= 3 && f[0] == name {
            return f[2].parse().ok();
        }
    }
    None
}

/// id→名稱（顯示用），找不到退回數字。
pub fn name_of(id: u32, group: bool) -> String {
    let path = if group { "/etc/group" } else { "/etc/passwd" };
    if let Ok(data) = fs::read_to_string(path) {
        for line in data.lines() {
            let f: Vec<&str> = line.split(':').collect();
            if f.len() >= 3 && f[2].parse::<u32>() == Ok(id) {
                return f[0].to_string();
            }
        }
    }
    id.to_string()
}

extern "C" {
    fn lchown(path: *const i8, uid: u32, gid: u32) -> i32;
    fn chown(path: *const i8, uid: u32, gid: u32) -> i32;
}

fn do_chown(path: &str, uid: Option<u32>, gid: Option<u32>, follow: bool) -> std::io::Result<()> {
    use std::ffi::CString;
    let c = CString::new(path).map_err(|_| std::io::Error::from_raw_os_error(22))?;
    let u = uid.unwrap_or(u32::MAX); // -1 = 不變更
    let g = gid.unwrap_or(u32::MAX);
    let f = if follow { chown } else { lchown };
    if unsafe { f(c.as_ptr(), u, g) } == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

pub fn run(
    prog: &str,
    files: &[String],
    opts: &ChOpts,
    uid: Option<u32>,
    gid: Option<u32>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> i32 {
    let mut rc = 0;
    for f in files {
        rc |= one(prog, f, opts, uid, gid, stdout, stderr, opts.recursive);
    }
    rc
}

#[allow(clippy::too_many_arguments)]
fn one(
    prog: &str,
    path: &str,
    opts: &ChOpts,
    mut uid: Option<u32>,
    mut gid: Option<u32>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
    nested: bool,
) -> i32 {
    if let Some(rf) = &opts.reference {
        match fs::metadata(rf) {
            Ok(m) => {
                if prog != "chgrp" {
                    uid = Some(m.uid());
                }
                gid = Some(m.gid());
            }
            Err(e) => {
                if !opts.silent {
                    writeln!(stderr, "{}: failed to get attributes of '{}': {}", prog, rf, crate::pathname::io_reason_pub(&e)).ok();
                }
                return 1;
            }
        }
    }
    let meta = if opts.traverse_symlinks {
        fs::metadata(path)
    } else {
        fs::symlink_metadata(path)
    };
    let meta = match meta {
        Ok(m) => m,
        Err(e) => {
            if !opts.silent {
                if opts.traverse_symlinks {
                    writeln!(stderr, "{}: cannot dereference '{}': {}", prog, path, crate::pathname::io_reason_pub(&e)).ok();
                } else {
                    writeln!(stderr, "{}: cannot access '{}': {}", prog, path, crate::pathname::io_reason_pub(&e)).ok();
                }
            }
            return 1;
        }
    };
    let old_uid = meta.uid();
    let old_gid = meta.gid();
    let is_dir = meta.is_dir();
    // 參考：chown ':grp' 的錯誤仍稱 ownership；只有 chgrp 用 group
    let what = if prog == "chgrp" { "group" } else { "ownership" };
    let res = do_chown(path, uid, gid, opts.traverse_symlinks);
    match res {
        Ok(()) => {
            let changed = (uid.is_some() && uid != Some(old_uid)) || (gid.is_some() && gid != Some(old_gid));
            if opts.verbose || (opts.changes && changed) {
                writeln!(
                    stdout,
                    "changed {} of '{}' from {} to {}",
                    what,
                    path,
                    fmt_ids2(prog, old_uid, old_gid),
                    fmt_ids2(prog, uid.unwrap_or(old_uid), gid.unwrap_or(old_gid))
                )
                .ok();
            }
        }
        Err(e) => {
            if !opts.silent {
                let reason = if nested {
                    crate::pathname::io_reason_nosuffix(&e)
                } else {
                    crate::pathname::io_reason_pub(&e)
                };
                writeln!(stderr, "{}: changing {} of '{}': {}", prog, what, path, reason).ok();
            }
            if opts.verbose {
                writeln!(
                    stderr,
                    "failed to change {} of '{}' from {} to {}",
                    what,
                    path,
                    fmt_ids2(prog, old_uid, old_gid),
                    fmt_ids2(prog, uid.unwrap_or(old_uid), gid.unwrap_or(old_gid))
                )
                .ok();
            }
            return 1;
        }
    }
    if opts.recursive && is_dir {
        if let Ok(rd) = fs::read_dir(path) {
            let mut entries: Vec<String> = rd.flatten().map(|e| e.path().to_string_lossy().into_owned()).collect();
            entries.sort();
            let mut sub_rc = 0;
            for p in entries {
                sub_rc |= one(prog, &p, opts, uid, gid, stdout, stderr, true);
            }
            return sub_rc;
        }
    }
    0
}

fn fmt_ids2(prog: &str, uid: u32, gid: u32) -> String {
    if prog == "chgrp" {
        return name_of(gid, true);
    }
    fmt_ids(uid, gid)
}

fn fmt_ids(uid: u32, gid: u32) -> String {
    format!("{}:{}", name_of(uid, false), name_of(gid, true))
}

pub struct Parsed {
    pub opts: ChOpts,
    pub files: Vec<String>,
    pub spec: Option<String>,
}

pub fn parse_common(prog: &str, args: &[String], stdout: &mut dyn Write, stderr: &mut dyn Write) -> Result<Parsed, i32> {
    let mut opts = ChOpts {
        traverse_symlinks: true,
        recursive: false,
        changes: false,
        verbose: false,
        silent: false,
        reference: None,
    };
    let mut files: Vec<String> = Vec::new();
    let mut spec: Option<String> = None;
    let mut i = 0usize;
    let mut only_operands = false;
    while i < args.len() {
        let a = args[i].as_str();
        if only_operands {
            files.push(a.to_string());
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
                "no-dereference" => opts.traverse_symlinks = false,
                "recursive" => opts.recursive = true,
                "changes" => opts.changes = true,
                "verbose" => opts.verbose = true,
                "silent" | "quiet" => opts.silent = true,
                "reference" => match getval(&mut i) {
                    Some(v) => opts.reference = Some(v),
                    None => {
                        writeln!(stderr, "{}: option requires an argument -- 'reference'", prog).ok();
                        return Err(2);
                    }
                },
                "help" => {
                    writeln!(stdout, "Usage: {} [OPTION]... OWNER[<.|:>GROUP] FILE...\nChange owner and/or group of each FILE", prog).ok();
                    return Err(0);
                }
                "version" => {
                    writeln!(stdout, "{} (uutils coreutils) 0.10.0", prog).ok();
                    return Err(0);
                }
                _ => {
                    writeln!(stderr, "{}: unrecognized option '{}'", prog, a).ok();
                    return Err(2);
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
                    'h' => opts.traverse_symlinks = false,
                    'H' => {} // 預設已追隨 operand symlink（uutils 亦為 no-op）
                    'L' => opts.traverse_symlinks = true,
                    'P' => opts.traverse_symlinks = false,
                    'R' => opts.recursive = true,
                    'c' => opts.changes = true,
                    'v' => opts.verbose = true,
                    'f' => opts.silent = true,
                    c => {
                        writeln!(stderr, "{}: invalid option -- '{}'", prog, c).ok();
                        return Err(2);
                    }
                }
                k += 1;
            }
            i += 1;
            continue;
        }
        if spec.is_none() {
            spec = Some(a.to_string());
        } else {
            files.push(a.to_string());
        }
        i += 1;
    }
    Ok(Parsed { opts, files, spec })
}
