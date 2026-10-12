//! whoami(1) — Print the current username.
//!
//! 依據 man page（docs/man/whoami.1.gz）與本機參考實測：
//! - 名稱由 effective uid 查 /etc/passwd（自行解析，見 src/passwd.rs）
//! - 查不到 → `whoami: failed to get username: No such id: <uid>`、rc 1（參考實測）
//! - 不受 LOGNAME 環境影響（參考實測：`env LOGNAME=zzz whoami` 仍印 uid 對應名稱）

use crate::passwd;
use crate::pathname::infer_long_opt;
use crate::sys;
use crate::{In, Out};

// 逐字取自參考 `whoami --help`。
const WHOAMI_HELP: &str = r#"Print the current username.

Usage: whoami

Options:
  -h, --help     Print help
  -V, --version  Print version"#;

const WHOAMI_LONGS: &[&str] = &["help", "version"];

/// whoami 沒有 positional，所以 clap 不印 tip 行（實測）。
fn clap_unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "Usage: whoami").ok();
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
            match infer_long_opt(&name, WHOAMI_LONGS) {
                Some("help") => {
                    writeln!(stdout, "{}", WHOAMI_HELP).ok();
                    return 0;
                }
                Some("version") => {
                    writeln!(stdout, "whoami (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => return clap_unexpected(a, stderr),
            }
        } else {
            for c in a[1..].chars() {
                match c {
                    'h' => {
                        writeln!(stdout, "{}", WHOAMI_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        writeln!(stdout, "whoami (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => return clap_unexpected(&format!("-{}", c), stderr),
                }
            }
        }
    }
    let uid = sys::geteuid();
    match passwd::passwd_by_uid(uid) {
        Some(p) => {
            writeln!(stdout, "{}", p.name).ok();
            0
        }
        None => {
            writeln!(stderr, "whoami: failed to get username: No such id: {}", uid).ok();
            1
        }
    }
}
