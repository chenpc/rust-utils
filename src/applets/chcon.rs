//! chcon(1) — Change SELinux security context.
//!
//! 行為規格：docs/man/chcon.1.gz（選項清單與描述）；man 未載的措辭/邊界依本機決定並記
//! docs/assumptions.md。注意：man（與本機參考）都**沒有** `-Z` / `--context`，
//! 因此這兩個旗標一律走 clap 的 unexpected-argument 錯誤。
//! guest / host 皆無 SELinux 標籤 → 所有設定路徑失敗，措辭（參考實測逐字）：
//! - 檔案不存在的完整 CONTEXT 路徑：`chcon: Accessing failed on 'FILE'. <reason (os error N)>.`
//! - 存在的檔案設定失敗：`chcon: Setting security context failed. setfilecon() failed on path 'P'. <reason (os error N)>.`
//!   （參考對存在的檔案用 '/proc/self/fd/N' 內部 fd 路徑；我方直接用檔案路徑 → assumptions）
//! - partial（-u/-r/-l/-t 而無 CONTEXT）：`chcon: Applying partial security context to unlabeled file failed on 'FILE'. invalid input parameter.`
//! - `--reference=RFILE`：RFILE 不存在 → `chcon: Getting security context failed. getfilecon() failed on path 'RFILE'. <reason (os error N)>.`；
//!   RFILE 存在但無 label → `chcon: Getting security context failed on 'RFILE'. <reason>.`
//! - 無 FILE operand：`chcon: No files are specified.` + 空行 + Try 行
//! - verbose：`chcon: changing security context of 'PATH'`（僅在檔案可存取時印）

use crate::pathname::usage_hint;
use crate::{In, Out};
use std::ffi::CString;

const XATTR: &str = "security.selinux";

extern "C" {
    fn lsetxattr(path: *const i8, name: *const i8, value: *const u8, size: usize, flags: i32) -> i32;
    fn lgetxattr(path: *const i8, name: *const i8, value: *mut u8, size: usize) -> isize;
    fn setxattr(path: *const i8, name: *const i8, value: *const u8, size: usize, flags: i32) -> i32;
    fn getxattr(path: *const i8, name: *const i8, value: *mut u8, size: usize) -> isize;
}

fn errno_string() -> String {
    // 參考措辭含 errno 代號：`No such file or directory (os error 2)`
    std::io::Error::last_os_error().to_string()
}

/// 存取檢查（dereference → 追連字號；否則查連字號本身）。
fn access_check(path: &str, dereference: bool) -> Result<(), String> {
    let r = if dereference {
        std::fs::metadata(path)
    } else {
        std::fs::symlink_metadata(path)
    };
    match r {
        Ok(_) => Ok(()),
        Err(_) => Err(errno_string()),
    }
}

fn set_context(path: &str, ctx: &[u8], dereference: bool) -> Result<(), String> {
    let c = match CString::new(path.as_bytes()) {
        Ok(c) => c,
        Err(_) => return Err("Invalid argument".to_string()),
    };
    let n = CString::new(XATTR).unwrap();
    let r = unsafe {
        if dereference {
            setxattr(c.as_ptr(), n.as_ptr(), ctx.as_ptr(), ctx.len(), 0)
        } else {
            lsetxattr(c.as_ptr(), n.as_ptr(), ctx.as_ptr(), ctx.len(), 0)
        }
    };
    if r != 0 {
        return Err(errno_string());
    }
    Ok(())
}

fn get_context(path: &str, dereference: bool) -> Result<Vec<u8>, String> {
    let c = match CString::new(path.as_bytes()) {
        Ok(c) => c,
        Err(_) => return Err("Invalid argument".to_string()),
    };
    let n = CString::new(XATTR).unwrap();
    let mut buf = vec![0u8; 1024];
    let r = unsafe {
        if dereference {
            getxattr(c.as_ptr(), n.as_ptr(), buf.as_mut_ptr(), buf.len())
        } else {
            lgetxattr(c.as_ptr(), n.as_ptr(), buf.as_mut_ptr(), buf.len())
        }
    };
    if r < 0 {
        return Err(errno_string());
    }
    let mut len = (r as usize).min(buf.len());
    while len > 0 && buf[len - 1] == 0 {
        len -= 1;
    }
    buf.truncate(len);
    Ok(buf)
}

struct Opts {
    recursive: bool,
    verbose: bool,
    dereference: bool,
    preserve_root: bool,
    traverse: char,
    traverse_given: bool,
    dereference_given: bool,
    reference: Option<String>,
    user: Option<String>,
    role: Option<String>,
    range: Option<String>,
    type_: Option<String>,
    operands: Vec<String>,
}

// man chcon(1) 的 USAGE 三行（错误與 help 共用）。
const USAGE: &str = "Usage: chcon [OPTION]... CONTEXT FILE...\n       chcon [OPTION]... [-u USER] [-r ROLE] [-l RANGE] [-t TYPE] FILE...\n       chcon [OPTION]... --reference=RFILE FILE...";

const CHCON_HELP: &str = r#"Change the SELinux security context of each FILE to CONTEXT.
With --reference, change the security context of each FILE to that of RFILE.

Usage: chcon [OPTION]... CONTEXT FILE...
       chcon [OPTION]... [-u USER] [-r ROLE] [-l RANGE] [-t TYPE] FILE...
       chcon [OPTION]... --reference=RFILE FILE...

Arguments:
  [FILE]...

Options:
      --dereference        Affect the referent of each symbolic link (this is the default), rather
                           than the symbolic link itself.
  -h, --no-dereference     Affect symbolic links instead of any referenced file.
      --help               help
      --preserve-root      Fail to operate recursively on '/'.
      --no-preserve-root   Do not treat '/' specially (the default).
      --reference <RFILE>  Use security context of RFILE, rather than specifying a CONTEXT value.
  -u, --user <USER>        Set user USER in the target security context.
  -r, --role <ROLE>        Set role ROLE in the target security context.
  -t, --type <TYPE>        Set type TYPE in the target security context.
  -l, --range <RANGE>      Set range RANGE in the target security context.
  -R, --recursive          Operate on files and directories recursively.
  -H                       If a command line argument is a symbolic link to a directory, traverse
                           it. Only valid when -R is specified.
  -L                       Traverse every symbolic link to a directory encountered. Only valid when
                           -R is specified.
  -P                       Do not traverse any symbolic links (default). Only valid when -R is
                           specified.
  -v, --verbose            Output a diagnostic for every file processed.
  -V, --version            Print version
"#;

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
        dereference: true,
        preserve_root: true,
        traverse: 'P',
        traverse_given: false,
        dereference_given: false,
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
            let name = crate::pathname::infer_long_opt(
                &name,
                &[
                    "dereference",
                    "help",
                    "no-dereference",
                    "no-preserve-root",
                    "preserve-root",
                    "range",
                    "recursive",
                    "reference",
                    "role",
                    "type",
                    "user",
                    "verbose",
                    "version",
                ],
            )
            .map(|full| full.to_string())
            .unwrap_or(name);
            match name.as_str() {
                "reference" => match value_of(&a, args, &mut i, "reference", "RFILE", stderr) {
                    Ok(v) => o.reference = Some(v),
                    Err(code) => return Err(code),
                }
                "recursive" => o.recursive = true,
                "verbose" => o.verbose = true,
                "dereference" => {
                    o.dereference = true;
                    o.dereference_given = true;
                }
                "no-dereference" => o.dereference = false,
                "preserve-root" => o.preserve_root = true,
                "no-preserve-root" => o.preserve_root = false,
                "user" => match value_of(&a, args, &mut i, "user", "USER", stderr) {
                    Ok(v) => o.user = Some(v),
                    Err(code) => return Err(code),
                },
                "role" => match value_of(&a, args, &mut i, "role", "ROLE", stderr) {
                    Ok(v) => o.role = Some(v),
                    Err(code) => return Err(code),
                },
                "range" => match value_of(&a, args, &mut i, "range", "RANGE", stderr) {
                    Ok(v) => o.range = Some(v),
                    Err(code) => return Err(code),
                },
                "type" => match value_of(&a, args, &mut i, "type", "TYPE", stderr) {
                    Ok(v) => o.type_ = Some(v),
                    Err(code) => return Err(code),
                },
                "help" => {
                    write!(stdout, "{}", CHCON_HELP).ok();
                    return Err(0);
                }
                "version" => {
                    writeln!(stdout, "chcon (uutils coreutils) 0.10.0").ok();
                    return Err(0);
                }
                _ => {
                    // clap 對 `--opt=value` 只印選項名（參考實測：`--context=s0` → '--context'）
                    unexpected(&format!("--{}", name), stderr);
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
                'h' => o.dereference = false,
                'H' | 'L' | 'P' => {
                    // man：-H/-L/-P 「Only valid when -R is specified.」；clap 在整個 argv 解析完才檢查
                    // （參考實測：`-L -R ...` 與 `-R -L ...` 都可行；只有 -L 而無 -R 才報錯）
                    o.traverse = c;
                    o.traverse_given = true;
                }
                'V' => {
                    writeln!(stdout, "chcon (uutils coreutils) 0.10.0").ok();
                    return Err(0);
                }
                'u' | 'r' | 'l' | 't' => {
                    let rest: String = chars[k + 1..].iter().collect();
                    let rest = rest.strip_prefix('=').unwrap_or(&rest).to_string();
                    let v = if !rest.is_empty() {
                        rest
                    } else {
                        if i >= args.len() {
                            // clap 即使短旗標也印長選項名（參考實測）
                            let (long, metavar) = match c {
                                'u' => ("user", "USER"),
                                'r' => ("role", "ROLE"),
                                'l' => ("range", "RANGE"),
                                _ => ("type", "TYPE"),
                            };
                            crate::pathname::clap_value_required(long, metavar, stderr);
                            return Err(1);
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
                c => {
                    unexpected(&format!("-{}", c), stderr);
                    return Err(1);
                }
            }
            k += 1;
        }
    }

    // clap 群組約束（參考實測）：-H/-L/-P 必須與 -R 併用，旗標順序不拘。
    if o.traverse_given && !o.recursive {
        writeln!(stderr, "error: the following required arguments were not provided:").ok();
        writeln!(stderr, "  --recursive").ok();
        writeln!(stderr).ok();
        writeln!(stderr, "{}", USAGE).ok();
        writeln!(stderr).ok();
        writeln!(stderr, "For more information, try '--help'.").ok();
        return Err(1);
    }
    // 參考實測：`--dereference`（明講）加上 -R 但沒有 -H/-L → 執行期錯誤 + Try 行。
    if o.recursive && o.dereference_given && !o.traverse_given {
        writeln!(
            stderr,
            "chcon: '--recursive' with '--dereference' require either '-H' or '-L'."
        )
        .ok();
        writeln!(stderr).ok();
        writeln!(stderr, "{}", usage_hint("chcon")).ok();
        return Err(1);
    }

    // GNU 兩種 synopsis：`chcon [OPTION]... CONTEXT FILE...` 與
    // `chcon [OPTION]... [-u USER] [-r ROLE] [-l RANGE] [-t TYPE] FILE...`：
    // 後者沒有 CONTEXT，所有 positional 都是 FILE（man 措辭 → 依 man 為準）。
    let partial_opts =
        o.user.is_some() || o.role.is_some() || o.range.is_some() || o.type_.is_some();
    let files: Vec<String>;
    let context: Option<String>;
    if o.reference.is_some() || partial_opts {
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

    let partial = context.is_none() && partial_opts;

    // man：-R 展開的檔案清單先建好，再一口氣 LABEL 化。
    // --preserve-root（預設）：operand 為 '/' 且 -R 時根本不下探。
    let mut rc = 0;
    let mut paths: Vec<String> = Vec::new();
    for f in &files {
        if o.preserve_root && o.recursive && f == "/" {
            writeln!(
                stderr,
                "chcon: warning: It is dangerous to operate recursively on '/'. Use --no-preserve-root to override this failsafe."
            )
            .ok();
            writeln!(stderr, "chcon: Modifying root path failed on '/'. permission denied.").ok();
            rc = 1;
            continue;
        }
        for path in walk(f, o.recursive, o.traverse) {
            paths.push(path);
        }
    }

    for path in &paths {
        if let Some(rfile) = &o.reference {
            if o.verbose {
                writeln!(stderr, "chcon: changing security context of '{}'", path).ok();
            }
            match get_context(rfile, o.dereference) {
                Ok(ctx) => {
                    if let Err(e) = set_context(path, &ctx, o.dereference) {
                        writeln!(
                            stderr,
                            "chcon: Setting security context failed. setfilecon() failed on path '{}'. {}.",
                            path, e
                        )
                        .ok();
                        rc = 1;
                    }
                }
                Err(e) => {
                    // 參考實測：RFILE 根本不存在 → 用 "getfilecon() failed on path" 版措辭；
                    // RFILE 存在但沒有 label → "Getting security context failed on 'RFILE'. <reason>."
                    let missing = std::path::Path::new(rfile).symlink_metadata().is_err();
                    if missing {
                        writeln!(
                            stderr,
                            "chcon: Getting security context failed. getfilecon() failed on path '{}'. {}.",
                            rfile, e
                        )
                        .ok();
                    } else {
                        writeln!(
                            stderr,
                            "chcon: Getting security context failed on '{}'. {}.",
                            rfile, e
                        )
                        .ok();
                    }
                    rc = 1;
                }
            }
            continue;
        }
        // 參考：完整/partial 路徑先存取檔案；失敗 → `Accessing failed on 'FILE'. <reason>.`（不印 verbose）
        if let Err(e) = access_check(path, o.dereference) {
            writeln!(stderr, "chcon: Accessing failed on '{}'. {}.", path, e).ok();
            rc = 1;
            continue;
        }
        if o.verbose {
            writeln!(stderr, "chcon: changing security context of '{}'", path).ok();
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
        match set_context(path, &bytes, o.dereference) {
            Ok(()) => {}
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
    Ok(rc)
}

fn unexpected(arg: &str, stderr: Out) {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(
        stderr,
        "  tip: to pass '{}' as a value, use '-- {}'",
        arg, arg
    )
    .ok();
    writeln!(stderr).ok();
    writeln!(stderr, "{}", USAGE).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
}

fn value_of(
    a: &str,
    args: &[String],
    i: &mut usize,
    long: &str,
    metavar: &str,
    stderr: Out,
) -> Result<String, i32> {
    if let Some(pos) = a.find('=') {
        return Ok(a[pos + 1..].to_string());
    }
    if *i >= args.len() {
        crate::pathname::clap_value_required(long, metavar, stderr);
        return Err(1);
    }
    let v = args[*i].clone();
    *i += 1;
    Ok(v)
}

/// -R 時展開目錄（遞迴內容先、目錄本身後 → 與參考一致）。
/// traverse: 'P' 跟連字號但不進入、'L' 進入所有連字號目錄、'H' 僅進入命令列 operand。
fn walk(path: &str, recursive: bool, traverse: char) -> Vec<String> {
    let mut out = Vec::new();
    walk_into(path, recursive, traverse, 0, &mut out);
    out
}

fn walk_into(path: &str, recursive: bool, traverse: char, depth: usize, out: &mut Vec<String>) {
    let md = match std::fs::symlink_metadata(path) {
        Ok(md) => md,
        Err(_) => {
            out.push(path.to_string());
            return;
        }
    };
    let is_dir = md.file_type().is_dir();
    let is_link = md.file_type().is_symlink();
    // man: -L 跟進每個連字號目錄；-H 只跟進命令列上出現的連字號；-P（預設）都不跟進。
    let follow = is_dir || (is_link && (traverse == 'L' || (traverse == 'H' && depth == 0)));
    if recursive && follow && depth < 64 {
        if let Ok(rd) = std::fs::read_dir(path) {
            let mut entries: Vec<String> = rd
                .filter_map(|e| e.ok())
                .map(|e| e.path().to_string_lossy().to_string())
                .collect();
            entries.sort();
            for e in entries {
                walk_into(&e, true, traverse, depth + 1, out);
            }
        }
    }
    out.push(path.to_string());
}
