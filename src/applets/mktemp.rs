//! mktemp(1) — Create a temporary file or directory.
//!
//! 行為依本機參考實作（uutils coreutils 0.10.0）逐 case diff（docs/probes/cluster5.md）：
//! - 預設模板 `tmp.` + 10 個 X；隨機字元集 [A-Za-z0-9]（讀 /dev/urandom）
//! - 模板需含至少 3 個連續 X，否則 "mktemp: too few X's in template 'T'"
//! - `--suffix=S` 接在隨機替換之後；模板不以 X 結尾又給 --suffix →
//!   "mktemp: with --suffix, template 'T' must end in X"；S 含 '/' →
//!   "mktemp: invalid suffix '/x', contains directory separator"
//! - `-p DIR` = `--tmpdir=DIR`（DIR 作為前綴，TEMPLATE 相對於它）；`--tmpdir` 無值 → $TMPDIR 或 /tmp；
//!   無 -p/--tmpdir 時 TEMPLATE 照原樣（相對於 cwd）
//! - `-t`（deprecated）：TMPDIR 或 /tmp 接 PREFIX，PREFIX 含 '/' →
//!   "mktemp: invalid template, 'T', contains directory separator"
//! - `-u/--dry-run` 只印不建立；`-d` 建立目錄；`-q` 抑制錯誤訊息（rc 仍 1）

use crate::{In, Out};
use std::io::Read;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};

struct Opts {
    directory: bool,
    dry_run: bool,
    quiet: bool,
    tmpdir: Option<String>,
    suffix: Option<String>,
    t_flag: bool,
    operands: Vec<String>,
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    match run_inner(args, stdin, stdout, stderr) {
        Ok(code) => code,
        Err(code) => code,
    }
}

fn run_inner(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> Result<i32, i32> {
    let mut o = Opts {
        directory: false,
        dry_run: false,
        quiet: false,
        tmpdir: None,
        suffix: None,
        t_flag: false,
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
                    "directory",
                    "dry-run",
                    "help",
                    "quiet",
                    "suffix",
                    "tmpdir",
                    "version",
                ],
            )
            .map(|full| full.to_string())
            .unwrap_or(name);
            match name.as_str() {
                "directory" | "dirs" => o.directory = true,
                "dry-run" => o.dry_run = true,
                "quiet" => o.quiet = true,
                "suffix" => {
                    o.suffix =
                        Some(long_value(&a, args, &mut i, "suffix", "SUFFIX", &o, stderr)?);
                }
                "tmpdir" => {
                    if let Some(pos) = a.find('=') {
                        o.tmpdir = Some(a[pos + 1..].to_string());
                    } else {
                        o.tmpdir = Some(String::new());
                    }
                }
                "help" => {
                    print_help(stdout);
                    return Err(0);
                }
                "version" => {
                    writeln!(stdout, "mktemp (uutils coreutils) 0.10.0").ok();
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
                'd' => o.directory = true,
                'u' => o.dry_run = true,
                'q' => o.quiet = true,
                't' => o.t_flag = true,
                'p' => {
                    let v = short_value(&rest, args, &mut i, 'p', &mut k, &o, stderr)?;
                    o.tmpdir = Some(v);
                }
                'h' => {
                    print_help(stdout);
                    return Err(0);
                }
                'V' => {
                    writeln!(stdout, "mktemp (uutils coreutils) 0.10.0").ok();
                    return Err(0);
                }
                c2 => {
                    return Err(unexpected(&format!("-{}", c2), stderr));
                }
            }
            k += 1;
        }
    }

    if o.operands.len() > 1 {
        if !o.quiet {
            writeln!(stderr, "mktemp: too many templates").ok();
            writeln!(
                stderr,
                "{}",
                crate::pathname::usage_hint("mktemp")
            )
            .ok();
        }
        return Err(1);
    }

    match build_name(&o, stdout, stderr) {
        Ok((name, template)) => {
            if o.dry_run {
                writeln!(stdout, "{}", name).ok();
                return Err(0);
            }
            match create(&name, o.directory) {
                Ok(()) => {
                    writeln!(stdout, "{}", name).ok();
                    Ok(0)
                }
                Err(e) => {
                    if !o.quiet {
                        // 規格：`mktemp: failed to create file|directory via template '<未隨機化模板>': <reason>`
                        let kind = if o.directory { "directory" } else { "file" };
                        writeln!(
                            stderr,
                            "mktemp: failed to create {} via template '{}': {}",
                            kind,
                            template,
                            strip_os_error(&e)
                        )
                        .ok();
                    }
                    Ok(1)
                }
            }
        }
        Err(code) => Err(code),
    }
}

/// 組出最終路徑（含隨機替換）。回 Err(1) 表示已印錯誤。
fn build_name(o: &Opts, _stdout: Out, stderr: Out) -> Result<(String, String), i32> {
    let quiet = o.quiet;
    let mut fail = |msg: String| -> i32 {
        if !quiet {
            writeln!(stderr, "mktemp: {}", msg).ok();
        }
        1
    };

    // 1. 決定 template（參考實測：`-t` 為 deprecated no-op，不改變語意）
    let template: String = match o.operands.first() {
        Some(t) => t.clone(),
        None => "tmp.XXXXXXXXXX".to_string(),
    };

    // 2. 決定基底目錄：無 --tmpdir/-p → TMPDIR 或 /tmp（僅預設模板生效）；
    //    有 --tmpdir/-p → 給定值（空值 → TMPDIR 或 /tmp）
    let path = if let Some(dir) = &o.tmpdir {
        let d = if dir.is_empty() { tmpdir_env() } else { dir.clone() };
        if template.starts_with('/') {
            return Err(fail(format!(
                "invalid template, '{}'; with --tmpdir, it may not be absolute",
                template
            )));
        }
        if d.is_empty() {
            template.clone()
        } else {
            format!("{}/{}", d.trim_end_matches('/'), template)
        }
    } else if o.t_flag {
        // GNU 把 -t 當成 no-op，參考實測則以 TMPDIR（或 /tmp）作為前綴；
        // X 數量檢查先於目錄分隔符錯誤（參考：mktemp -t 'sub/dir.X'）
        if !has_run_of_x(&template, 3) {
            return Err(fail(format!("too few X's in template '{}'", template)));
        }
        if template.contains('/') {
            return Err(fail(format!(
                "invalid template, '{}', contains directory separator",
                template
            )));
        }
        format!(
            "{}/{}",
            tmpdir_env().trim_end_matches('/'),
            template
        )
    } else if o.operands.is_empty() {
        format!("{}/{}", tmpdir_env(), template)
    } else {
        path_strip_tmp(&template, o)
    };

    // 3. suffix 規則
    let suffix = if let Some(s) = &o.suffix {
        if s.contains('/') {
            return Err(fail(format!(
                "invalid suffix '{}', contains directory separator",
                s
            )));
        }
        if !template.ends_with('X') {
            return Err(fail(format!(
                "with --suffix, template '{}' must end in X",
                template
            )));
        }
        Some(s.clone())
    } else if !template.ends_with('X') && o.operands.first().is_some() {
        // TEMPLATE 不以 X 結尾：隱含 suffix（參考：--suffix implied）
        None
    } else {
        None
    };

    // 4. X 檢查與隨機替換
    let final_path = match replace_x(&path, suffix.as_deref()) {
        Ok(v) => v,
        Err(ReplErr::FewX(t)) => {
            let shown = if o.tmpdir.is_some() { base_name(&t) } else { t };
            return Err(fail(format!("too few X's in template '{}'", shown)));
        }
        Err(ReplErr::NoX(t)) => {
            let shown = if o.tmpdir.is_some() { base_name(&t) } else { t };
            return Err(fail(format!("too few X's in template '{}'", shown)));
        }
    };
    let _ = o;
    Ok((final_path, path))
}

/// 去掉 io::Error Display 尾端的 ` (os error N)`（規格訊息只留 reason）
fn strip_os_error(msg: &str) -> String {
    match msg.rfind(" (os error ") {
        Some(i) => msg[..i].to_string(),
        None => msg.to_string(),
    }
}

enum ReplErr {
    FewX(String),
    NoX(String),
}

/// 找「最後一段 X run」（至少 3 個）並就地替換為同長度隨機 alnum。
/// 參考實測：run 不必在結尾（`dXXXXtail` → `d<rand>tail`），但長度 < 3 即報
/// "too few X's"（`aXXXbXX` 雖然前面有 3 個 X，最後一段只有 2 個 → 錯誤）。
fn replace_x(path: &str, suffix: Option<&str>) -> Result<String, ReplErr> {
    let chars: Vec<char> = path.chars().collect();
    let last_x = match chars.iter().rposition(|c| *c == 'X') {
        Some(p) => p,
        None => return Err(ReplErr::NoX(path.to_string())),
    };
    let mut start = last_x + 1;
    while start > 0 && chars[start - 1] == 'X' {
        start -= 1;
    }
    let run = last_x + 1 - start;
    if run < 3 {
        return Err(ReplErr::FewX(path.to_string()));
    }
    let mut out: String = chars[..start].iter().collect();
    out.push_str(&rand_name(run));
    out.extend(chars[last_x + 1..].iter());
    if let Some(s) = suffix {
        out.push_str(s);
    }
    Ok(out)
}

fn base_name(p: &str) -> String {
    crate::pathname::base_part(p).to_string()
}

/// 最後一段連續 'X' 是否有 n 個以上（與 replace_x 的判定一致）。
fn has_run_of_x(s: &str, n: usize) -> bool {
    let chars: Vec<char> = s.chars().collect();
    let last_x = match chars.iter().rposition(|c| *c == 'X') {
        Some(p) => p,
        None => return false,
    };
    let mut start = last_x + 1;
    while start > 0 && chars[start - 1] == 'X' {
        start -= 1;
    }
    last_x + 1 - start >= n
}

fn tmpdir_env() -> String {
    std::env::var("TMPDIR")
        .ok()
        .filter(|v| !v.is_empty())
        .unwrap_or_else(|| "/tmp".to_string())
}

/// 無 -p/-t 時：TEMPLATE 照原樣使用（TMPDIR 不生效）。
fn path_strip_tmp(template: &str, _o: &Opts) -> String {
    template.to_string()
}

fn rand_name(n: usize) -> String {
    const ALNUM: &[u8] = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789";
    let mut bytes = vec![0u8; n];
    match std::fs::File::open("/dev/urandom") {
        Ok(mut f) => {
            if f.read_exact(&mut bytes).is_err() {
                fill_fallback(&mut bytes);
            }
        }
        Err(_) => fill_fallback(&mut bytes),
    }
    let mut out = String::with_capacity(n);
    for i in 0..n {
        let mut v = bytes[i];
        if v >= 62 {
            v %= 62;
        }
        out.push(ALNUM[v as usize] as char);
    }
    out
}

fn fill_fallback(bytes: &mut [u8]) {
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9e3779b97f4a7c15);
    let mut s = seed ^ (bytes.len() as u64).wrapping_mul(0x100000001b3);
    for b in bytes.iter_mut() {
        s ^= s << 13;
        s ^= s >> 7;
        s ^= s << 17;
        *b = (s % 62) as u8;
    }
}

fn create(path: &str, directory: bool) -> Result<(), String> {
    if directory {
        match std::fs::DirBuilder::new().mode(0o700).create(path) {
            Ok(()) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    } else {
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
        {
            Ok(_) => Ok(()),
            Err(e) => Err(e.to_string()),
        }
    }
}

fn print_help(stdout: Out) {
    write!(
        stdout,
        r#"Create a temporary file or directory.

Usage: mktemp [OPTION]... [TEMPLATE]

Arguments:
  [template]  

Options:
  -d, --directory        Make a directory instead of a file
  -u, --dry-run          do not create anything; merely print a name (unsafe)
  -q, --quiet            Fail silently if an error occurs.
      --suffix <SUFFIX>  append SUFFIX to TEMPLATE; SUFFIX must not contain a path separator. This
                         option is implied if TEMPLATE does not end with X.
  -p <DIR>               short form of --tmpdir
      --tmpdir[=<DIR>]   interpret TEMPLATE relative to DIR; if DIR is not specified, use $TMPDIR
                         ($TMP on windows) if set, else /tmp. With this option, TEMPLATE must not be
                         an absolute name; unlike with -t, TEMPLATE may contain slashes, but mktemp
                         creates only the final component
  -t                     Generate a template (using the supplied prefix and TMPDIR (TMP on windows)
                         if set) to create a filename template [deprecated]
  -h, --help             Print help
  -V, --version          Print version
"#
    )
    .ok();
}

fn long_value(
    a: &str,
    args: &[String],
    i: &mut usize,
    long: &str,
    metavar: &str,
    o: &Opts,
    stderr: Out,
) -> Result<String, i32> {
    if let Some(pos) = a.find('=') {
        return Ok(a[pos + 1..].to_string());
    }
    if *i >= args.len() {
        // 參考實測：措辭用該 option 的 metavar（<SUFFIX>），且 rc = 1（不是 clap 預設的 2）
        if !o.quiet {
            crate::pathname::clap_value_required(long, metavar, stderr);
        }
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
    o: &Opts,
    stderr: Out,
) -> Result<String, i32> {
    if !rest.is_empty() {
        // `-p=` 是「空值」（clap 語意）；`-pDIR` 則是attached 值
        let v = if let Some(v) = rest.strip_prefix('=') {
            v.to_string()
        } else {
            rest.to_string()
        };
        *k = usize::MAX - 1;
        return Ok(v);
    }
    if *i >= args.len() {
        if !o.quiet {
            crate::pathname::clap_value_required(&ch.to_string(), "DIR", stderr);
        }
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
    if arg.starts_with('-') {
        writeln!(stderr, "  tip: to pass '{}' as a value, use '-- {}'", arg, arg).ok();
        writeln!(stderr).ok();
    }
    writeln!(stderr, "Usage: mktemp [OPTION]... [TEMPLATE]").ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}
