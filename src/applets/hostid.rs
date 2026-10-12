//! hostid(1) — Print the numeric identifier (in hexadecimal) for the current host.
//!
//! 依據 man page（docs/man/hostid.1.gz）與本機參考實測：
//! - 輸出為 gethostid(2) 的 8 位小寫十六進位（實測 `007f0101`）
//! - man 未描述任何選項（只有 -h/-V）；無 `-a` 寫入模式（記錄於 docs/assumptions.md）
//! - 任何 operand 或未知選項 → clap 風格錯誤且無 tip 行（實測）

use crate::pathname::infer_long_opt;
use crate::sys;
use crate::{In, Out};

// 逐字取自參考 `hostid --help`（Usage 是 `hostid [options]`）。
const HOSTID_HELP: &str = r#"Print the numeric identifier (in hexadecimal) for the current host

Usage: hostid [options]

Options:
  -h, --help     Print help
  -V, --version  Print version"#;

const HOSTID_LONGS: &[&str] = &["help", "version"];

fn clap_unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "Usage: hostid [options]").ok();
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
            match infer_long_opt(&name, HOSTID_LONGS) {
                Some("help") => {
                    writeln!(stdout, "{}", HOSTID_HELP).ok();
                    return 0;
                }
                Some("version") => {
                    writeln!(stdout, "hostid (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => return clap_unexpected(a, stderr),
            }
        } else {
            for c in a[1..].chars() {
                match c {
                    'h' => {
                        writeln!(stdout, "{}", HOSTID_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        writeln!(stdout, "hostid (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => return clap_unexpected(&format!("-{}", c), stderr),
                }
            }
        }
    }
    writeln!(stdout, "{:08x}", (sys::gethostid_i32() as u32) & 0xFFFF_FFFF).ok();
    0
}
