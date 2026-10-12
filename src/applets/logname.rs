//! logname(1) — Print user's login name.
//!
//! 依據 man page（docs/man/logname.1.gz）與本機參考實測：
//! - 優先順序：getlogin(3) → 由 uid 查 /etc/passwd（參考在無 utmp 的沙箱內仍能由
//!   自訂 /etc/passwd 印出名稱，證明有這條退徑）
//! - 不受 LOGNAME 環境影響（參考實測：`env LOGNAME=zzz logname` 仍印實際名稱）
//! - 全部失敗 → `logname: no login name`、rc 1（參考實測）

use crate::passwd;
use crate::pathname::infer_long_opt;
use crate::sys;
use crate::{In, Out};

// 逐字取自參考 `logname --help`。
const LOGNAME_HELP: &str = r#"Print user's login name

Usage: logname

Options:
  -h, --help     Print help
  -V, --version  Print version"#;

const LOGNAME_LONGS: &[&str] = &["help", "version"];

fn clap_unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "Usage: logname").ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut no_more_opts = false;
    for a in args {
        if no_more_opts || (!a.starts_with('-') || a == "-") {
            return clap_unexpected(a, stderr);
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            match infer_long_opt(&name, LOGNAME_LONGS) {
                Some("help") => {
                    writeln!(stdout, "{}", LOGNAME_HELP).ok();
                    return 0;
                }
                Some("version") => {
                    writeln!(stdout, "logname (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => return clap_unexpected(a, stderr),
            }
        } else {
            for c in a[1..].chars() {
                match c {
                    'h' => {
                        writeln!(stdout, "{}", LOGNAME_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        writeln!(stdout, "logname (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => return clap_unexpected(&format!("-{}", c), stderr),
                }
            }
        }
    }
    let name = match sys::login_name() {
        Some(n) if !n.is_empty() => Some(n),
        _ => passwd::passwd_by_uid(sys::getuid()).map(|p| p.name),
    };
    match name {
        Some(n) => {
            writeln!(stdout, "{}", n).ok();
            0
        }
        None => {
            writeln!(stderr, "logname: no login name").ok();
            1
        }
    }
}
