//! printenv(1) — Display the values of the specified environment VARIABLE(s).
//!
//! 依據 man page（docs/man/printenv.1.gz）：
//! `  -0, --null     end each output line with 0 byte rather than newline`
//! operand 為 `VARIABLE` 清單；無 operand 時印出全部 name=value。
//!
//! 參考實測（本機 uutils coreutils 0.10.0）：
//! - 無 operand → 依 environ 原始順序印 NAME=VALUE（不排序）
//! - 有 operand → 只印存在的變數值；任一不存在 → rc1、stderr 空
//! - 未知選項 → clap 措辭，rc=2
//! - `--` 之後一律視為 operand

use crate::pathname::infer_long_opt;
use crate::{In, Out};

// 注意：`  [variables]...  ` 這行在參考 help 中有尾隨兩個空白（逐字保留）。
const PRINTENV_HELP: &str = r#"Display the values of the specified environment VARIABLE(s), or (with no VARIABLE) display name and
value pairs for them all.

Usage: printenv [OPTION]... [VARIABLE]...

Arguments:
  [variables]...  

Options:
  -0, --null     end each output line with 0 byte rather than newline
  -h, --help     Print help
  -V, --version  Print version"#;

const PRINTENV_LONGS: &[&str] = &["help", "null", "version"];

fn clap_unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "  tip: to pass '{}' as a value, use '-- {}'", arg, arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "Usage: printenv [OPTION]... [VARIABLE]...").ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    2
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut nul = false;
    let mut nul_given = false;
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
            match infer_long_opt(&name, PRINTENV_LONGS) {
                Some("help") => {
                    writeln!(stdout, "{}", PRINTENV_HELP).ok();
                    return 0;
                }
                Some("version") => {
                    writeln!(stdout, "printenv (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                Some("null") => {
                    if nul_given {
                        return crate::pathname::clap_dup("null", "", 2, stderr);
                    }
                    nul_given = true;
                    nul = true;
                }
                _ => return clap_unexpected(&a, stderr),
            }
            continue;
        }
        for c in a[1..].chars() {
            match c {
                '0' => {
                    if nul_given {
                        return crate::pathname::clap_dup("null", "", 2, stderr);
                    }
                    nul_given = true;
                    nul = true;
                }
                'h' => {
                    writeln!(stdout, "{}", PRINTENV_HELP).ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "printenv (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => return clap_unexpected(&format!("-{}", c), stderr),
            }
        }
    }

    // 用 lossy 轉換而非 env::vars()，避免非 UTF-8 環境讓 in-process 測試 panic。
    let table: Vec<(String, String)> = std::env::vars_os()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.to_string_lossy().into_owned(),
            )
        })
        .collect();

    let mut rc = 0;
    if operands.is_empty() {
        for (k, v) in &table {
            if nul {
                write!(stdout, "{}={}\u{0}", k, v).ok();
            } else {
                writeln!(stdout, "{}={}", k, v).ok();
            }
        }
    } else {
        for name in &operands {
            match table.iter().find(|(k, _)| k == name) {
                Some((_, v)) => {
                    if nul {
                        write!(stdout, "{}\u{0}", v).ok();
                    } else {
                        writeln!(stdout, "{}", v).ok();
                    }
                }
                None => rc = 1,
            }
        }
    }
    rc
}
