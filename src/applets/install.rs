//! install(1) — Copy files and set modes/ownership.
//!
//! 行為依本機參考實作（uutils coreutils 0.10.0）逐 case diff（docs/probes/cluster5.md）：
//! - 複製檔預設 mode 755（chmod 顯式套用，不受程序 umask 影響）；copy 後先 chmod 0o600
//!   （參考實測：`-o root` 失敗留在 dst 的半成品 mode 恒為 600），chown 失敗則後面的
//!   chmod 不執行
//! - `-d`/`-D` 預設 mode 0o777 & !umask 且**顯式 chmod**（參考在 umask 0 下仍產 755）
//! - `-m` 錯誤措辭 "install: Invalid mode string: <reason>"（大寫 I）
//! - 缺 operand："install: missing file operand" /
//!   "install: missing destination file operand after 'SRC'" /
//!   "install: install with -d requires at least one argument."（皆接 Try 行）
//! - 同檔："install: 'SRC' and 'DST' are the same file"；目錄 SRC："install: omitting directory 'D'"
//! - `-C`：內容相同 → 不複製內容，但 mode/ownership 仍套用
//! - `-S SUFFIX` 單獨使用即啟用備份；`--backup=numbered` → name.~N~
//! - `-v`：覆寫既有 DST 先印 "removed 'DST'"，再印 "'SRC' -> 'DST'"；`-d` 印 "creating directory 'X'"
//! - `-t DIR`：DIR 不存在時建立之（uutils 行為 → assumptions）
//! - `-s` 呼叫外部 strip；失敗 "install: strip program failed: strip process terminated abnormally - exit code: N"
//! - `-o/-g` 名稱查 /etc/passwd、/etc/group；"install: invalid user: 'x'"

use crate::applets::chmod::{current_umask, parse_mode_chmod};
use crate::chown_common::lookup_name;
use crate::pathname::{io_reason_pub, usage_hint};
use crate::{In, Out};
use std::fs;
use std::os::unix::fs::{DirBuilderExt, MetadataExt, PermissionsExt};
use std::process::Command;

enum Backup {
    None,
    Simple(String),
    Numbered,
}

struct Cfg {
    dirs: bool,
    make_parents: bool,
    target_dir: Option<String>,
    no_target_dir: bool,
    mode: Option<u32>,
    uid: Option<u32>,
    gid: Option<u32>,
    preserve_times: bool,
    compare: bool,
    strip: bool,
    verbose: bool,
    backup: Backup,
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    match run_inner(args, stdin, stdout, stderr) {
        Ok(code) => code,
        Err(code) => code,
    }
}

fn run_inner(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> Result<i32, i32> {
    let mut c = Cfg {
        dirs: false,
        make_parents: false,
        target_dir: None,
        no_target_dir: false,
        mode: None,
        uid: None,
        gid: None,
        preserve_times: false,
        compare: false,
        strip: false,
        verbose: false,
        backup: Backup::None,
    };
    let mut operands: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        i += 1;
        if no_more_opts || (!a.starts_with('-') || a == "-") {
            operands.push(a);
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            match name.as_str() {
                "directory" => c.dirs = true,
                "directory-to-suffix" => {
                    // uutils 不接受 --directory-to-suffix 作為長選項
                    return Err(unexpected(&a, stderr));
                }
                "target-directory" => {
                    c.target_dir = Some(long_value(&a, args, &mut i, "target-directory", stderr)?);
                }
                "no-target-directory" => c.no_target_dir = true,
                "mode" => {
                    let spec = long_value(&a, args, &mut i, "mode", stderr)?;
                    c.mode = Some(match parse_install_mode(&spec, stderr) {
                        Some(m) => m,
                        None => return Err(1),
                    });
                }
                "group" => {
                    let spec = long_value(&a, args, &mut i, "group", stderr)?;
                    match parse_id(&spec, true, stderr) {
                        Some(v) => c.gid = Some(v),
                        None => return Err(1),
                    }
                }
                "owner" => {
                    let spec = long_value(&a, args, &mut i, "owner", stderr)?;
                    match parse_id(&spec, false, stderr) {
                        Some(v) => c.uid = Some(v),
                        None => return Err(1),
                    }
                }
                "preserve-timestamps" => c.preserve_times = true,
                "compare" => c.compare = true,
                "strip" => c.strip = true,
                "verbose" => c.verbose = true,
                "suffix" => {
                    let s = long_value(&a, args, &mut i, "suffix", stderr)?;
                    c.backup = Backup::Simple(s);
                }
                "backup" => {
                    let how = if let Some(pos) = a.find('=') {
                        a[pos + 1..].to_string()
                    } else {
                        String::new()
                    };
                    c.backup = match how.as_str() {
                        "" => Backup::Simple(backup_suffix()),
                        "simple" | "none" | "existing" => Backup::Simple(backup_suffix()),
                        "numbered" => Backup::Numbered,
                        _ => {
                            writeln!(stderr, "install: invalid backup type: '{}'", how).ok();
                            return Err(1);
                        }
                    };
                }
                "help" => {
                    print_help(stdout);
                    return Err(0);
                }
                "version" => {
                    writeln!(stdout, "install (uutils coreutils) 0.10.0").ok();
                    return Err(0);
                }
                _ => return Err(unexpected(&a, stderr)),
            }
            continue;
        }
        let chars: Vec<char> = a[1..].chars().collect();
        let mut k = 0usize;
        while k < chars.len() {
            let ch = chars[k];
            let rest: String = chars[k + 1..].iter().collect();
            match ch {
                'd' => c.dirs = true,
                'D' => c.make_parents = true,
                'C' => c.compare = true,
                'c' => {}
                'p' => c.preserve_times = true,
                's' => c.strip = true,
                'v' => c.verbose = true,
                'T' => c.no_target_dir = true,
                'Z' => {}
                't' => {
                    let v = short_value(&rest, args, &mut i, 't', &mut k, stderr)?;
                    c.target_dir = Some(v);
                }
                'm' => {
                    let spec = short_value(&rest, args, &mut i, 'm', &mut k, stderr)?;
                    c.mode = Some(match parse_install_mode(&spec, stderr) {
                        Some(m) => m,
                        None => return Err(1),
                    });
                }
                'o' => {
                    let spec = short_value(&rest, args, &mut i, 'o', &mut k, stderr)?;
                    match parse_id(&spec, false, stderr) {
                        Some(v) => c.uid = Some(v),
                        None => return Err(1),
                    }
                }
                'g' => {
                    let spec = short_value(&rest, args, &mut i, 'g', &mut k, stderr)?;
                    match parse_id(&spec, true, stderr) {
                        Some(v) => c.gid = Some(v),
                        None => return Err(1),
                    }
                }
                'S' => {
                    let s = short_value(&rest, args, &mut i, 'S', &mut k, stderr)?;
                    c.backup = Backup::Simple(s);
                }
                'b' => c.backup = Backup::Simple(backup_suffix()),
                'h' => {
                    print_help(stdout);
                    return Err(0);
                }
                'V' => {
                    writeln!(stdout, "install (uutils coreutils) 0.10.0").ok();
                    return Err(0);
                }
                c2 => {
                    writeln!(stderr, "install: invalid option -- '{}'", c2).ok();
                    writeln!(stderr, "Try 'install --help' for more information.").ok();
                    return Err(2);
                }
            }
            k += 1;
        }
    }

    if c.compare && c.strip {
        writeln!(
            stderr,
            "install: Options --compare and --strip are mutually exclusive"
        )
        .ok();
        writeln!(stderr, "{}", usage_hint("install")).ok();
        return Err(1);
    }

    if c.dirs {
        if operands.is_empty() {
            writeln!(
                stderr,
                "install: install with -d requires at least one argument."
            )
            .ok();
            writeln!(stderr, "{}", usage_hint("install")).ok();
            return Err(1);
        }
        // 參考實測：目錄 mode 恒為 0o777 & !umask（顯式 chmod，不受程序 umask 影響）；
        // 显式 -m 只補葉層
        let def_masked = 0o777u32 & !current_umask();
        let mut rc = 0;
        for d in &operands {
            if let Err(e) = mkdir_p(d, def_masked, c.mode, c.verbose, stdout) {
                writeln!(stderr, "install: cannot create directory '{}': {}", d, e).ok();
                rc = 1;
            }
        }
        return Ok(rc);
    }

    if operands.is_empty() {
        writeln!(stderr, "install: missing file operand").ok();
        writeln!(stderr, "{}", usage_hint("install")).ok();
        return Err(1);
    }

    // 決定 (sources, dest)
    let sources: Vec<String>;
    let dest: String;
    if let Some(td) = &c.target_dir {
        sources = operands.clone();
        dest = td.clone();
    } else if operands.len() == 1 {
        writeln!(
            stderr,
            "install: missing destination file operand after '{}'",
            operands[0]
        )
        .ok();
        writeln!(stderr, "{}", usage_hint("install")).ok();
        return Err(1);
    } else if c.no_target_dir {
        if operands.len() > 2 {
            writeln!(stderr, "install: extra operand '{}'", operands[2]).ok();
            writeln!(stderr, "install [OPTION]... [FILE]...").ok();
            return Err(1);
        }
        sources = vec![operands[0].clone()];
        dest = operands[1].clone();
    } else {
        dest = operands[operands.len() - 1].clone();
        sources = operands[..operands.len() - 1].to_vec();
    }

    // -t：參考實測 —— 目標不存在時**不建立目錄**；單一檔案直接把目標當成檔案，
    // 多個檔案則回報 target 不是目錄
    if c.target_dir.is_some() && !std::path::Path::new(&dest).is_dir() && operands.len() > 1 {
        writeln!(stderr, "install: target '{}' is not a directory", dest).ok();
        writeln!(stderr, "{}", usage_hint("install")).ok();
        return Err(1);
    }
    if c.make_parents {
        if let Some(parent) = std::path::Path::new(&dest).parent() {
            let ps = parent.to_string_lossy().to_string();
            if !ps.is_empty() && !std::path::Path::new(&ps).is_dir() {
                if let Err(e) = mkdir_p(
                    &ps,
                    0o777u32 & !current_umask(),
                    None,
                    c.verbose,
                    stdout,
                ) {
                    writeln!(stderr, "install: cannot create directory '{}': {}", ps, e).ok();
                    return Err(1);
                }
            }
        }
    }

    let into_dir = std::path::Path::new(&dest).is_dir();
    let mode = c.mode.unwrap_or(0o755);
    let mut rc = 0;
    for src in &sources {
        let md = match fs::symlink_metadata(src) {
            Ok(m) => m,
            Err(e) => {
                writeln!(stderr, "install: cannot stat '{}': {}", src, io_reason_pub(&e)).ok();
                rc = 1;
                continue;
            }
        };
        if md.is_dir() {
            writeln!(stderr, "install: omitting directory '{}'", src).ok();
            rc = 1;
            continue;
        }
        let target = if into_dir {
            let base = crate::pathname::base_part(src);
            format!("{}/{}", dest.trim_end_matches('/'), base)
        } else {
            dest.clone()
        };
        if same_file(src, &target) {
            writeln!(stderr, "install: '{}' and '{}' are the same file", src, target).ok();
            rc = 1;
            continue;
        }
        if copy_one(src, &target, &c, mode, stdout, stderr).is_err() {
            rc = 1;
            continue;
        }
        if c.strip {
            match Command::new("strip").arg(&target).status() {
                Ok(status) if status.success() => {}
                Ok(status) => {
                    let code = status.code().unwrap_or(1);
                    writeln!(
                        stderr,
                        "install: strip program failed: strip process terminated abnormally - exit code: {}",
                        code
                    )
                    .ok();
                    let _ = fs::remove_file(&target);
                    rc = 1;
                    continue;
                }
                Err(_) => {
                    writeln!(
                        stderr,
                        "install: strip program failed: strip process terminated abnormally - exit code: 1"
                    )
                    .ok();
                    let _ = fs::remove_file(&target);
                    rc = 1;
                    continue;
                }
            }
        }
        if c.verbose {
            writeln!(stdout, "'{}' -> '{}'", src, target).ok();
        }
    }
    Ok(rc)}

fn copy_one(
    src: &str,
    dst: &str,
    c: &Cfg,
    mode: u32,
    stdout: Out,
    stderr: Out,
) -> Result<(), ()> {
    let existing = fs::symlink_metadata(dst).ok();
    if c.compare {
        if let Some(em) = &existing {
            if em.is_file() {
                if let (Ok(a), Ok(b)) = (fs::read(src), fs::read(dst)) {
                    if a == b {
                        // 內容相同：不複製，但仍套用 mode / ownership / timestamps
                        apply_post(src, dst, c, mode, None, stderr)?;
                        return Ok(());
                    }
                }
            }
        }
    }
    let old = match &existing {
        Some(em) if em.is_file() || em.is_symlink() => Some(dst.to_string()),
        _ => None,
    };
    // 備份（先保留舊檔），再移除舊檔、寫新檔
    let backup_taken = match (&c.backup, &old) {
        (Backup::Simple(suffix), Some(o)) => {
            let name = format!("{}{}", o, suffix);
            if fs::rename(o, &name).is_ok() {
                Some(name)
            } else {
                None
            }
        }
        (Backup::Numbered, Some(o)) => {
            let name = numbered_name(o);
            if fs::rename(o, &name).is_ok() {
                Some(name)
            } else {
                None
            }
        }
        _ => None,
    };
    let _ = backup_taken;
    if old.is_some() && c.verbose {
        writeln!(stdout, "removed '{}'", dst).ok();
        let _ = fs::remove_file(dst);
    } else if old.is_some() {
        let _ = fs::remove_file(dst);
    }
    if let Err(e) = fs::copy(src, dst) {
        writeln!(stderr, "install: cannot copy '{}' to '{}': {}", src, dst, io_reason_pub(&e)).ok();
        return Err(());
    }
    // 參考實測：copy 之後、chown/chmod 之前，dst 的 mode 是 0o600
    let _ = fs::set_permissions(dst, fs::Permissions::from_mode(0o600));
    apply_post(src, dst, c, mode, existing, stderr)?;
    Ok(())
}

fn apply_post(
    src: &str,
    dst: &str,
    c: &Cfg,
    mode: u32,
    _old: Option<fs::Metadata>,
    stderr: Out,
) -> Result<(), ()> {
    if c.uid.is_some() || c.gid.is_some() {
        extern "C" {
            fn chown(path: *const i8, uid: u32, gid: u32) -> i32;
        }
        let uid = c.uid.unwrap_or(u32::MAX);
        let gid = c.gid.unwrap_or(u32::MAX);
        let cpath = match std::ffi::CString::new(dst.as_bytes()) {
            Ok(v) => v,
            Err(_) => return Err(()),
        };
        if unsafe { chown(cpath.as_ptr(), uid, gid) } != 0 {
            let e = std::io::Error::last_os_error();
            writeln!(
                stderr,
                "install: failed to chown '{}': changing ownership of '{}': {}.",
                dst,
                dst,
                io_reason_pub(&e)
            )
            .ok();
            return Err(());
        }
    }
    // 參考實測：ownership 失敗時 mode 不會被套用（chown 在前、chmod 在後）
    if let Err(e) = fs::set_permissions(dst, fs::Permissions::from_mode(mode)) {
        writeln!(stderr, "install: cannot set mode for '{}': {}", dst, io_reason_pub(&e)).ok();
        return Err(());
    }
    if c.preserve_times {
        if let Ok(md) = fs::symlink_metadata(src) {
            set_times(dst, md.atime(), md.mtime(), md.atime_nsec(), md.mtime_nsec());
        }
    }
    Ok(())
}

fn set_times(path: &str, atime: i64, mtime: i64, atime_nsec: i64, mtime_nsec: i64) {
    extern "C" {
        #[link_name = "utimensat"]
        fn utimensat_install(dirfd: i32, path: *const i8, times: *const [i64; 4], flags: i32) -> i32;
    }
    let c = match std::ffi::CString::new(path.as_bytes()) {
        Ok(c) => c,
        Err(_) => return,
    };
    let times = [atime, atime_nsec, mtime, mtime_nsec];
    unsafe {
        utimensat_install(-100, c.as_ptr(), &times, 0);
    }
}

fn same_file(a: &str, b: &str) -> bool {
    let (ma, mb) = (fs::metadata(a), fs::metadata(b));
    match (ma, mb) {
        (Ok(x), Ok(y)) => x.ino() == y.ino() && x.dev() == y.dev(),
        _ => {
            let na = std::path::Path::new(a).canonicalize().ok();
            let nb = std::path::Path::new(b).canonicalize().ok();
            match (na, nb) {
                (Some(x), Some(y)) => x == y,
                _ => false,
            }
        }
    }
}

fn numbered_name(path: &str) -> String {
    let dir = std::path::Path::new(path)
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let base = crate::pathname::base_part(path).to_string();
    let mut max = 0u64;
    if let Ok(rd) = fs::read_dir(if dir.is_empty() { "." } else { &dir }) {
        for e in rd.filter_map(|e| e.ok()) {
            let name = e.file_name().to_string_lossy().to_string();
            if let Some(rest) = name.strip_prefix(&base) {
                if rest.starts_with(".~") && rest.ends_with("~") {
                    if let Ok(n) = rest[2..rest.len() - 1].parse::<u64>() {
                        if n > max {
                            max = n;
                        }
                    }
                }
            }
        }
    }
    format!("{}.~{}~", path, max + 1)
}

pub fn backup_suffix() -> String {
    std::env::var("SIMPLE_BACKUP_SUFFIX").unwrap_or_else(|_| "~".to_string())
}

fn mkdir_p(
    path: &str,
    def_mode: u32,
    explicit: Option<u32>,
    verbose: bool,
    stdout: Out,
) -> Result<(), String> {
    let p = std::path::Path::new(path);
    if p.is_dir() {
        return Ok(());
    }
    let mut cur = std::path::PathBuf::new();
    for comp in crate::pathname::components(path) {
        if comp.is_empty() {
            continue;
        }
        cur.push(comp);
        if cur.is_dir() {
            continue;
        }
        let is_leaf = {
            let rest = crate::pathname::components(path).len();
            let done = crate::pathname::components(&cur.to_string_lossy()).len();
            done >= rest
        };
        // 參考實測：中間層恒為 0o777 & !umask；显式 -m 只套葉層
        let m = if is_leaf { explicit.unwrap_or(def_mode) } else { def_mode };
        match fs::DirBuilder::new().mode(0o777).create(&cur) {
            Ok(()) => {
                let _ = fs::set_permissions(&cur, fs::Permissions::from_mode(m));
                if verbose {
                    writeln!(
                        stdout,
                        "creating directory '{}'",
                        cur.to_string_lossy()
                    )
                    .ok();
                }
            }
            Err(e) => return Err(io_reason_pub(&e)),
        }
    }
    let _ = p;
    Ok(())
}

fn parse_id(spec: &str, group: bool, stderr: Out) -> Option<u32> {
    if let Ok(v) = spec.parse::<u32>() {
        return Some(v);
    }
    match lookup_name(spec, group) {
        Some(v) => Some(v),
        None => {
            if group {
                writeln!(stderr, "install: invalid group: '{}'", spec).ok();
            } else {
                writeln!(stderr, "install: invalid user: '{}'", spec).ok();
            }
            None
        }
    }
}

/// install 的 -m：octal 精確或 symbolic；措辭 "install: Invalid mode string: <reason>"
fn parse_install_mode(spec: &str, stderr: Out) -> Option<u32> {
    if spec.chars().all(|c| c.is_ascii_digit()) && !spec.is_empty() {
        match u32::from_str_radix(spec, 8) {
            Ok(v) => return Some(v),
            Err(_) => {
                writeln!(
                    stderr,
                    "install: Invalid mode string: invalid digit found in string"
                )
                .ok();
                return None;
            }
        }
    }
    match parse_mode_chmod(spec) {
        Ok(s) => Some(apply_spec(&s)),
        Err(e) => {
            writeln!(stderr, "install: Invalid mode string: {}", e).ok();
            None
        }
    }
}

fn apply_spec(spec: &crate::applets::chmod::Spec) -> u32 {
    use crate::applets::chmod::Op;
    // 參考實測：install 的 symbolic mode 以 0 為基底（-m a+x -> 111、-m u+x -> 100）
    let mut perm = 0o0u32;
    for op in &spec.0 {
        match op {
            Op::Octal(m) => perm = *m,
            Op::Sym(who, oper, perms) => match oper {
                '+' => perm |= who & perms,
                '-' => perm &= !(who & perms),
                _ => perm = (perm & !who) | (who & perms),
            },
        }
    }
    perm
}

fn print_help(stdout: Out) {
    writeln!(
        stdout,
        "Copy SOURCE to DEST or multiple SOURCE(s) to the existing
DIRECTORY, while setting permission modes and owner/group

Usage: install [OPTION]... [FILE]...

Arguments:
  [files]...  

Options:
  -b, --backup[=METHOD]  backup each destination file before overwriting
  -C, --compare          compare each destination file with source and only replace when they differ
  -c                   ignored (for compatibility with GNU install)
  -d, --directory      create each directory and its parent directories
  -D, --make-directories  create leading directories of DEST
  -g, --group <GROUP>  set group ownership, translated from the GROUP name to the group ID
  -m, --mode <MODE>    set mode to MODE, instead of u+rw and not a=rw - umask for -d
  -o, --owner <OWNER>  set ownership (super user only)
  -p, --preserve-timestamps  do not modify access/modification times of receiving files
  -S, --suffix <SUFFIX>  override the usual backup suffix
  -s, --strip          run the strip program on each installed file to save space
  -t, --target-directory <DIR>  copy all SOURCE arguments into DIRECTORY
  -T, --no-target-directory  treat DEST as a normal file
  -v, --verbose        print the names of files as they are installed
  -h, --help           Print help
  -V, --version        Print version"
    )
    .ok();
}

fn long_value(a: &str, args: &[String], i: &mut usize, long: &str, stderr: Out) -> Result<String, i32> {
    if let Some(pos) = a.find('=') {
        return Ok(a[pos + 1..].to_string());
    }
    if *i >= args.len() {
        crate::pathname::clap_value_required(long, "VALUE", stderr);
        return Err(1);
    }
    let v = args[*i].clone();
    *i += 1;
    Ok(v)
}

fn short_value(
    rest: &str,
    args: &[String],
    i: &mut usize,
    ch: char,
    k: &mut usize,
    stderr: Out,
) -> Result<String, i32> {
    if !rest.is_empty() {
        // 吃掉剩餘 cluster（不可用 usize::MAX：呼叫端 k += 1 會溢位 panic）
        *k = usize::MAX - 1;
        return Ok(rest.to_string());
    }
    if *i >= args.len() {
        crate::pathname::clap_value_required(&ch.to_string(), "VALUE", stderr);
        return Err(1);
    }
    let v = args[*i].clone();
    *i += 1;
    *k = usize::MAX - 1;
    Ok(v)
}

fn unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(
        stderr,
        "  tip: to pass '{}' as a value, use '-- {}'",
        arg, arg
    )
    .ok();
    writeln!(stderr).ok();
    writeln!(stderr, "Usage: install [OPTION]... [FILE]...").ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}
