// mv — 搬移／重新命名（GNU man 9.7 ＋ 參考黑箱探測）
use crate::pathname::io_reason_nosuffix;
use std::fs;
use std::os::unix::fs::MetadataExt;
use std::path::Path;

#[derive(Clone, Copy, PartialEq)]
enum When {
    Never,
    Once,
    Always,
}

struct Cfg {
    backup: When,
    suffix: String,
    force: bool,
    interactive: When,
    no_clobber: bool,
    update: bool,
    target_dir: Option<String>,
    no_target_dir: bool,
    strip_slashes: bool,
    verbose: bool,
    exchange: bool,
    no_copy: bool,
}

fn help(prog: &str, stdout: crate::Out) {
    writeln!(
        stdout,
        "Usage: {} [OPTION]... [-T] SOURCE DEST
  or:  {} [OPTION]... SOURCE... DIRECTORY
  or:  {} [OPTION]... -t DIRECTORY SOURCE...
Rename SOURCE to DEST, or move SOURCE(s) to DIRECTORY.

      --backup[=CONTROL]       make a backup of each existing destination file
  -b                           like --backup but does not accept an argument
  -f, --force                  do not prompt before overwriting
  -i, --interactive            prompt before overwrite
  -n, --no-clobber             do not overwrite an existing file
      --strip-trailing-slashes  remove any trailing slashes from each SOURCE
                                 argument
  -S, --suffix=SUFFIX          override the usual backup suffix
  -t, --target-directory=DIRECTORY  move all SOURCE arguments into DIRECTORY
  -T, --no-target-directory   treat DEST as a normal file
  -u, --update                 move only when the SOURCE file is newer
                                 than the destination file or when the
                                 destination file is missing
  -v, --verbose                explain what is being done
      --exchange               exchange source and destination (atomic swap)
      --no-copy                fail instead of falling back to copy on
                                 cross-device moves
  -Z                           set SELinux context (not supported)
      --help     display this help and exit
      --version  output version information and exit", prog, prog, prog
    )
    .ok();
}

pub fn run(
    args: &[String],
    stdin: crate::In,
    stdout: crate::Out,
    stderr: crate::Out,
) -> i32 {
    let mut c = Cfg {
        backup: When::Never,
        suffix: std::env::var("SIMPLE_BACKUP_SUFFIX").unwrap_or_else(|_| "~".to_string()),
        force: false,
        interactive: When::Never,
        no_clobber: false,
        update: false,
        target_dir: None,
        no_target_dir: false,
        strip_slashes: false,
        verbose: false,
        exchange: false,
        no_copy: false,
    };
    let mut stdin = stdin;
    let mut operands: Vec<String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "-h" | "--help" => {
                help("mv", stdout);
                return 0;
            }
            "-V" | "--version" => {
                writeln!(stdout, "mv (uutils coreutils) 0.10.0").ok();
                return 0;
            }
            "-b" => c.backup = When::Always,
            "-f" | "--force" => c.force = true,
            "-i" => c.interactive = When::Always,
            "--interactive" => c.interactive = When::Always,
            s if s.starts_with("--interactive=") => {
                match &s["--interactive=".len()..] {
                    "never" => c.interactive = When::Never,
                    "always" | "prompt" => c.interactive = When::Always,
                    other => {
                        writeln!(stderr, "mv: invalid argument '{}' for '--interactive'", other).ok();
                        return 2;
                    }
                }
            }
            "-I" => c.interactive = When::Once,
            "-n" | "--no-clobber" => c.no_clobber = true,
            "-u" | "--update" => c.update = true,
            "-v" | "--verbose" => c.verbose = true,
            "-T" | "--no-target-directory" => c.no_target_dir = true,
            "--strip-trailing-slashes" => c.strip_slashes = true,
            "--exchange" => c.exchange = true,
            "--no-copy" => c.no_copy = true,
            "-Z" => {
                writeln!(stderr, "mv: SELinux support is not available").ok();
                return 1;
            }
            "-S" | "--suffix" => {
                i += 1;
                c.suffix = match args.get(i) {
                    Some(v) => v.clone(),
                    None => {
                        writeln!(stderr, "mv: option requires an argument -- 'S'").ok();
                        return 2;
                    }
                };
            }
            "-t" | "--target-directory" => {
                i += 1;
                c.target_dir = match args.get(i) {
                    Some(v) => Some(v.clone()),
                    None => {
                        writeln!(stderr, "mv: option requires an argument -- 't'").ok();
                        return 2;
                    }
                };
            }
            "--backup" => c.backup = When::Always,
            _ if a.starts_with("--suffix=") => c.suffix = a["--suffix=".len()..].to_string(),
            _ if a.starts_with("--target-directory=") => {
                c.target_dir = Some(a["--target-directory=".len()..].to_string());
            }
            _ if a.starts_with("--backup=") => {
                let v = &a["--backup=".len()..];
                if v == "none" || v == "off" {
                    c.backup = When::Never;
                } else {
                    c.backup = When::Always;
                }
            }
            _ if a.starts_with('-') && a.len() > 1 && !a.starts_with("--") => {
                let chars: Vec<char> = a[1..].chars().collect();
                let mut k = 0;
                while k < chars.len() {
                    match chars[k] {
                        'b' => c.backup = When::Always,
                        'f' => c.force = true,
                        'i' => c.interactive = When::Always,
                        'I' => c.interactive = When::Once,
                        'n' => c.no_clobber = true,
                        'u' => c.update = true,
                        'v' => c.verbose = true,
                        'T' => c.no_target_dir = true,
                        'S' | 't' => {
                            let rest: String = chars[k + 1..].iter().collect();
                            let val = if !rest.is_empty() {
                                rest
                            } else {
                                i += 1;
                                match args.get(i) {
                                    Some(v) => v.clone(),
                                    None => {
                                        writeln!(
                                            stderr,
                                            "mv: option requires an argument -- '{}'",
                                            chars[k]
                                        )
                                        .ok();
                                        return 2;
                                    }
                                }
                            };
                            if chars[k] == 'S' {
                                c.suffix = val;
                            } else {
                                c.target_dir = Some(val);
                            }
                            k = chars.len();
                            continue;
                        }
                        'Z' => {
                            writeln!(stderr, "mv: SELinux support is not available").ok();
                            return 1;
                        }
                        _ => {
                            writeln!(stderr, "mv: invalid option -- '{}'", chars[k]).ok();
                            return 2;
                        }
                    }
                    k += 1;
                }
            }
            _ if a.starts_with("--") => {
                writeln!(stderr, "mv: unrecognized option '{}'", a).ok();
                return 2;
            }
            "-" => operands.push("-".to_string()),
            _ => operands.push(a.to_string()),
        }
        i += 1;
    }

    if c.interactive == When::Always && c.force {
        c.interactive = When::Never;
    }

    // 決定 (sources, dest)
    let mut srcs: Vec<String>;
    let dest: String;
    if let Some(td) = &c.target_dir {
        if operands.is_empty() {
            writeln!(stderr, "mv: missing destination file operand").ok();
            return 1;
        }
        srcs = operands.clone();
        dest = td.clone();
    } else if operands.is_empty() {
        writeln!(stderr, "mv: missing file operand").ok();
        return 1;
    } else if operands.len() == 1 {
        writeln!(
            stderr,
            "mv: missing destination file operand after '{}'",
            operands[0]
        )
        .ok();
        return 1;
    } else if c.no_target_dir {
        srcs = vec![operands[0].clone()];
        dest = operands[1].clone();
        if operands.len() > 2 {
            writeln!(stderr, "mv: extra operand '{}'", operands[2]).ok();
            return 2;
        }
    } else {
        dest = operands[operands.len() - 1].clone();
        srcs = operands[..operands.len() - 1].to_vec();
    }

    if c.strip_slashes {
        srcs = srcs
            .iter()
            .map(|s| {
                let t = s.trim_end_matches('/');
                if t.is_empty() {
                    "/".to_string()
                } else {
                    t.to_string()
                }
            })
            .collect();
    }

    let dest_is_dir = c.target_dir.is_some() || (!c.no_target_dir && fs::metadata(&dest).map(|m| m.is_dir()).unwrap_or(false));
    // 多 operand 但終點不是目錄：GNU 只報一次 target 錯誤
    if c.target_dir.is_none() && !c.no_target_dir && operands.len() > 2 && !dest_is_dir {
        match fs::metadata(&dest) {
            Err(e) => {
                writeln!(stderr, "mv: target '{}': {}", dest, io_reason_nosuffix(&e)).ok();
                return 1;
            }
            Ok(_) => {
                writeln!(stderr, "mv: target '{}': Not a directory", dest).ok();
                return 1;
            }
        }
    }

    let mut rc = 0;
    for src in &srcs {
        let d = if dest_is_dir {
            let base = base_name(src);
            if base.is_empty() {
                writeln!(stderr, "mv: missing destination of '{}'", src).ok();
                rc = 1;
                continue;
            }
            format!("{}/{}", dest.trim_end_matches('/'), base)
        } else {
            dest.clone()
        };
        if let Err(e) = move_one(src, &d, &c, &mut stdin, stderr) {
            let _ = e;
            rc = 1;
        }
    }
    rc
}

fn base_name(p: &str) -> String {
    let t = p.trim_end_matches('/');
    match t.rsplit('/').next() {
        Some(b) if !b.is_empty() && b != "." => b.to_string(),
        _ => String::new(),
    }
}

fn within(src: &str, dir: &str) -> bool {
    // src（詞彙化）是否為 dir 的嚴格祖先
    let s = format!("{}", Path::new(src).display()).trim_end_matches('/').to_string();
    let d = format!("{}", Path::new(dir).display()).trim_end_matches('/').to_string();
    if s.is_empty() || d.is_empty() {
        return false;
    }
    d.len() > s.len() && d.as_bytes()[..s.len()] == s.as_bytes()[..] && d.as_bytes()[s.len()] == b'/'
}

fn move_one(src: &str, dst: &str, c: &Cfg, stdin: &mut crate::In, stderr: crate::Out) -> Result<(), ()> {
    if src == "-" && false {
        let _ = stdin;
    }
    let _ = stdin;
    let smd = match fs::symlink_metadata(src) {
        Ok(m) => m,
        Err(e) => {
            writeln!(stderr, "mv: cannot stat '{}': {}", src, io_reason_nosuffix(&e)).ok();
            return Err(());
        }
    };

    // 參考行為：dest 以 '/' 結尾但不是目錄 → failed to access ... Not a directory
    if dst.ends_with('/') && !Path::new(dst).is_dir() {
        writeln!(stderr, "mv: failed to access '{}': Not a directory", dst).ok();
        return Err(());
    }

    let backup_will_run = c.backup != When::Never && fs::symlink_metadata(dst).is_ok();
    // 同檔檢查（backup 會先移開 dest 時，延後到 backup 之後）
    if !backup_will_run {
        if let Ok(dmd) = fs::symlink_metadata(dst) {
            if smd.dev() == dmd.dev() && smd.ino() == dmd.ino() {
                writeln!(stderr, "mv: '{}' and '{}' are the same file", src, dst).ok();
                return Err(());
            }
        }
    }

    // 移入自身子目錄
    if smd.is_dir() && within(src, dst) {
        writeln!(
            stderr,
            "mv: cannot move '{}' to a subdirectory of itself, '{}'",
            src, dst
        )
        .ok();
        return Err(());
    }

    let dst_exists = fs::symlink_metadata(dst).is_ok();
    let interactive = c.interactive;
    if dst_exists {
        if c.no_clobber {
            return Ok(());
        }
        if c.update {
            // 來源較舊 → 略過
            if let (Ok(s2), Ok(d2)) = (fs::symlink_metadata(src), fs::symlink_metadata(dst)) {
                let snan = super::cp::mtime_ns_of(&s2);
                let dnan = super::cp::mtime_ns_of(&d2);
                if snan <= dnan {
                    return Ok(());
                }
            }
        }
        if interactive == When::Always {
            write!(stderr, "mv: overwrite '{}'? ", dst).ok();
            let line = super::cp::read_line(stdin);
            if !matches!(line.trim().chars().next(), Some('y') | Some('Y')) {
                return Err(());
            }
        }
        if c.backup != When::Never {
            let bak = format!("{}{}", dst, c.suffix);
            let _ = fs::remove_file(&bak);
            if let Err(e) = fs::rename(dst, &bak) {
                writeln!(
                    stderr,
                    "mv: cannot make backup of '{}': {}",
                    dst,
                    io_reason_nosuffix(&e)
                )
                .ok();
                return Err(());
            }
            if c.verbose {
                writeln!(stderr, "renamed '{}' -> '{}' (backup: '{}')", src, dst, bak).ok();
            }
            if let Ok(dmd) = fs::symlink_metadata(dst) {
                if smd.dev() == dmd.dev() && smd.ino() == dmd.ino() {
                    writeln!(stderr, "mv: '{}' and '{}' are the same file", src, dst).ok();
                    return Err(());
                }
            }
            return do_rename(src, dst, &smd, c, true, true, stderr);
        }
        if c.force {
            let _ = remove_dst(dst);
        }
    }
    do_rename(src, dst, &smd, c, !c.force, false, stderr)
}

fn remove_dst(dst: &str) -> std::io::Result<()> {
    match fs::symlink_metadata(dst) {
        Ok(m) if m.is_dir() => fs::remove_dir(dst),
        _ => fs::remove_file(dst),
    }
}

fn do_rename(
    src: &str,
    dst: &str,
    smd: &fs::Metadata,
    c: &Cfg,
    fail_if_exists: bool,
    logged: bool,
    stderr: crate::Out,
) -> Result<(), ()> {
    if c.exchange {
        // renameat2 RENAME_EXCHANGE = 2
        use std::ffi::CString;
        extern "C" {
            fn renameat2(olddirfd: i32, oldpath: *const i8, newdirfd: i32, newpath: *const i8, flags: u32) -> i32;
        }
        let cs = match CString::new(src.as_bytes()) {
            Ok(x) => x,
            Err(_) => return Err(()),
        };
        let cd = match CString::new(dst.as_bytes()) {
            Ok(x) => x,
            Err(_) => return Err(()),
        };
        let r = unsafe { renameat2(-100, cs.as_ptr(), -100, cd.as_ptr(), 2) };
        if r != 0 {
            let e = std::io::Error::last_os_error();
            writeln!(stderr, "mv: cannot move '{}': {}", dst, io_reason_nosuffix(&e)).ok();
            return Err(());
        }
        if c.verbose {
            writeln!(stderr, "renamed '{}' -> '{}'", src, dst).ok();
        }
        return Ok(());
    }
    match fs::rename(src, dst) {
        Ok(()) => {
            if c.verbose && !logged {
                writeln!(stderr, "renamed '{}' -> '{}'", src, dst).ok();
            }
            Ok(())
        }
        Err(e) => {
            let code = e.raw_os_error().unwrap_or(0);
            // ENOTEMPTY / EISDIR：dst 是非空目錄
            if (code == 21 || code == 39 || code == 17) && fail_if_exists {
                writeln!(
                    stderr,
                    "mv: cannot move '{}' to '{}': {}",
                    src,
                    dst,
                    io_reason_nosuffix(&e)
                )
                .ok();
                return Err(());
            }
            if code == 18 /* EXDEV */ {
                if c.no_copy {
                    writeln!(
                        stderr,
                        "mv: cannot move '{}': Invalid cross-device link",
                        src
                    )
                    .ok();
                    return Err(());
                }
                // 跨裝置：copy + unlink
                if let Err(()) = super::cp::mv_copy_fallback(src, dst, stderr) {
                    writeln!(
                        stderr,
                        "mv: cannot move '{}' to '{}': copy failed",
                        src, dst
                    )
                    .ok();
                    return Err(());
                }
                if c.verbose {
                    writeln!(stderr, "renamed '{}' -> '{}'", src, dst).ok();
                }
                let _ = smd;
                return Ok(());
            }
            writeln!(
                stderr,
                "mv: cannot move '{}' to '{}': {}",
                src,
                dst,
                io_reason_nosuffix(&e)
            )
            .ok();
            Err(())
        }
    }
}
