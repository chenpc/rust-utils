//! mkfifo(1) — Create FIFO files.
//!
//! 行為依本機參考實作（uutils coreutils 0.10.0）逐 case diff（docs/probes/cluster5.md）：
//! - 預設 mode = 0o666 & !umask（guest umask 0 → 666）
//! - `-m MODE` octal/symbolic；錯誤措辭 "mkfifo: invalid mode: <reason>"
//! - 建立失敗 "mkfifo: cannot create fifo 'P': <reason>"（無 "(os error N)" 後綴）
//! - 缺 operand：clap 措辭 "error: the following required arguments were not provided:" + "  <fifo>..."
//! - `-Z/--context`：接受但 no-op（無 SELinux，見 assumptions）

use crate::applets::chmod::parse_mode_chmod;
use crate::pathname::io_reason_nosuffix;
use crate::{In, Out};
use std::ffi::CString;
use std::os::unix::fs::PermissionsExt;

fn current_umask() -> u32 {
    extern "C" {
        fn umask(mask: u32) -> u32;
    }
    let m = unsafe { umask(0) };
    unsafe { umask(m) };
    m
}

// 參考實測：REF 的 --help 全文（逐字，含換行與對齊）。
const MKFIFO_HELP: &str = r#"Create a FIFO with the given name.

Usage: mkfifo [OPTION]... NAME...

Options:
  -m, --mode <MODE>      file permissions for the fifo
  -Z                     set the SELinux security context to default type
      --context[=<CTX>]  like -Z, or if CTX is specified then set the SELinux or SMACK security
                         context to CTX
  -h, --help             Print help
  -V, --version          Print version"#;

fn usage_line() -> &'static str {
    "mkfifo [OPTION]... NAME..."
}

fn required_args(stderr: Out) -> i32 {
    writeln!(stderr, "error: the following required arguments were not provided:").ok();
    writeln!(stderr, "  <fifo>...").ok();
    writeln!(stderr).ok();
    writeln!(stderr, "Usage: {}", usage_line()).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut mode: Option<u32> = None;
    // clap 先報缺少的 positional；mode 解析錯誤延到 NAME 檢查之後（參考實測）
    let mut mode_spec: Option<crate::applets::chmod::Spec> = None;
    let mut mode_err: Option<String> = None;
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
            let name = crate::pathname::infer_long_opt(
                &name,
                &["context", "help", "mode", "version"],
            )
            .map(|full| full.to_string())
            .unwrap_or(name);
            match name.as_str() {
                "context" => {
                    // --context[=CTX]：有 '=' 就吞值，否則不吞
                    continue;
                }
                "help" => {
                    writeln!(stdout, "{}", MKFIFO_HELP).ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "mkfifo (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                "mode" => {
                    let spec = if let Some(pos) = a.find('=') {
                        a[pos + 1..].to_string()
                    } else {
                        if i >= args.len() {
                            crate::pathname::clap_value_required("mode", "MODE", stderr);
                            return 1;
                        }
                        let v = args[i].clone();
                        i += 1;
                        v
                    };
                    match parse_mode_chmod(&spec) {
                        Ok(s) => mode_spec = Some(s),
                        Err(e) => mode_err = Some(e),
                    }
                    continue;
                }
                _ => {
                    writeln!(stderr, "error: unexpected argument '{}' found", a).ok();
                    writeln!(stderr).ok();
                    writeln!(
                        stderr,
                        "  tip: to pass '{}' as a value, use '-- {}'",
                        a, a
                    )
                    .ok();
                    writeln!(stderr).ok();
                    writeln!(stderr, "Usage: {}", usage_line()).ok();
                    writeln!(stderr).ok();
                    writeln!(stderr, "For more information, try '--help'.").ok();
                    return 1;
                }
            }
        }
        // short cluster
        let chars: Vec<char> = a[1..].chars().collect();
        let mut k = 0usize;
        while k < chars.len() {
            match chars[k] {
                'm' => {
                    let rest: String = chars[k + 1..].iter().collect();
                    let rest = rest.strip_prefix('=').unwrap_or(&rest).to_string();
                    let spec = if !rest.is_empty() {
                        rest
                    } else {
                        if i >= args.len() {
                            // 參考對 `-m`（無值）也用長名稱
                            crate::pathname::clap_value_required("mode", "MODE", stderr);
                            return 1;
                        }
                        let v = args[i].clone();
                        i += 1;
                        v
                    };
                    k = chars.len();
                    match parse_mode_chmod(&spec) {
                        Ok(s) => mode_spec = Some(s),
                        Err(e) => mode_err = Some(e),
                    }
                }
                'Z' => {
                    // man: `-Z [CTX]` 值可省略；附值時整串當 CTX 吞掉
                    // （guest 無 SELinux → 僅接受、不套用 label）
                    let rest: String = chars[k + 1..].iter().collect();
                    if !rest.is_empty() {
                        let _ctx = rest.strip_prefix('=').unwrap_or(&rest).to_string();
                        k = chars.len();
                    }
                }
                'h' => {
                    writeln!(stdout, "{}", MKFIFO_HELP).ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "mkfifo (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => {
                    writeln!(stderr, "error: unexpected argument '-{}' found", c).ok();
                    writeln!(stderr).ok();
                    writeln!(
                        stderr,
                        "  tip: to pass '-{}' as a value, use '-- -{}'",
                        c, c
                    )
                    .ok();
                    writeln!(stderr).ok();
                    writeln!(stderr, "Usage: {}", usage_line()).ok();
                    writeln!(stderr).ok();
                    writeln!(stderr, "For more information, try '--help'.").ok();
                    return 1;
                }
            }
            k += 1;
        }
    }

    if operands.is_empty() {
        return required_args(stderr);
    }
    if let Some(e) = mode_err {
        writeln!(stderr, "mkfifo: invalid mode: {}", e).ok();
        return 1;
    }
    if let Some(s) = &mode_spec {
        mode = Some(apply_spec(s));
    }

    let base = mode.unwrap_or(0o666 & !current_umask());
    let mut failed = 0;
    for p in &operands {
        let c = match CString::new(p.as_bytes()) {
            Ok(c) => c,
            Err(_) => {
                writeln!(stderr, "mkfifo: cannot create fifo '{}': Invalid argument", p).ok();
                failed = 1;
                continue;
            }
        };
        extern "C" {
            fn mkfifo(path: *const i8, mode: u32) -> i32;
        }
        // umask 只套預設 mode；顯式 -m 需事後 chmod（mkfifo(2) 本身會遮 umask）
        let m = if mode.is_some() { base } else { base & !current_umask() };
        if unsafe { mkfifo(c.as_ptr(), m) } != 0 {
            let e = std::io::Error::last_os_error();
            writeln!(
                stderr,
                "mkfifo: cannot create fifo '{}': {}",
                p,
                io_reason_nosuffix(&e)
            )
            .ok();
            failed = 1;
            continue;
        }
        if let Some(perm) = mode {
            if let Err(e) =
                std::fs::set_permissions(p, std::fs::Permissions::from_mode(perm))
            {
                writeln!(
                    stderr,
                    "mkfifo: cannot create fifo '{}': {}",
                    p,
                    io_reason_nosuffix(&e)
                )
                .ok();
                failed = 1;
            }
        }
    }
    failed
}

fn apply_spec(spec: &crate::applets::chmod::Spec) -> u32 {
    use crate::applets::chmod::Op;
    // 純 symbolic 以 a=rw (0666) 為基底（同 mknod）
    let mut perm = 0o666u32;
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
