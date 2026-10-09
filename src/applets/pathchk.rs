//! pathchk(1) — Check file name validity and portability.
//!
//! 依據 man page（docs/man/pathchk.1.gz）+ GNU coreutils 9.5 info：
//! - 預設檢查：既有目錄 component 缺 search 權限、NAME 超過 OS 上限、component 超過檔案系統上限
//! - `-p`：改用 POSIX 檢查（空名稱、非可移植字元、超過 POSIX 最小可移植上限）
//! - `-P`：空名稱或 component 以 `-` 開頭
//! - `--portability` = `-p -P`
//! - 不存在的 NAME 不是錯誤；全部通過 → exit 0，否則 1
//! - 選項必須在 operand 之前

use crate::pathname::{self as p, CheckOpts};
use crate::{In, Out};

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut o = CheckOpts::default();
    let mut operands: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    for a in args {
        if no_more_opts || !a.starts_with('-') || a == "-" {
            operands.push(a.clone());
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            match &a[2..] {
                "portability" => {
                    o.posix = true;
                    o.leading_hyphen = true;
                }
                "help" => {
                    writeln!(
                        stdout,
                        "Usage: pathchk [-p] [-P] [--portability] [-h|--help] [-V|--version] [path]"
                    )
                    .ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "pathchk (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                name => {
                    writeln!(stderr, "pathchk: unrecognized option '--{name}'").ok();
                    return 2;
                }
            }
            continue;
        }
        for c in a[1..].chars() {
            match c {
                'p' => o.posix = true,
                'P' => o.leading_hyphen = true,
                'h' => {
                    writeln!(
                        stdout,
                        "Usage: pathchk [-p] [-P] [--portability] [-h|--help] [-V|--version] [path]"
                    )
                    .ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "pathchk (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => {
                    writeln!(stderr, "pathchk: invalid option -- '{c}'").ok();
                    return 2;
                }
            }
        }
    }
    if operands.is_empty() {
        writeln!(stderr, "pathchk: missing operand").ok();
        return 1;
    }
    let mut status = 0;
    for name in &operands {
        for reason in p::check_name(name, o) {
            writeln!(stderr, "{reason}").ok();
            status = 1;
        }
    }
    status
}
