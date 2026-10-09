//! false(1) — Returns false, an unsuccessful exit status.
//!
//! 依據 man page（docs/man/false.1.gz）：立即以 exit status 1 離開。
//! 若帶 `--help` / `--version`，會嘗試寫出 help/version 文字；即使寫入發生 IO 錯誤，
//! 仍然回傳 1。

use crate::{In, Out};

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    for a in args {
        if a == "--help" || a == "-h" {
            if writeln!(stdout, "Usage: false [--help] [--version]").is_err() {
                writeln!(stderr, "false: cannot write help text").ok();
            }
            return 1;
        }
        if a == "--version" || a == "-V" {
            if writeln!(stdout, "false (rust-coreutils) 0.1.0").is_err() {
                writeln!(stderr, "false: cannot write version text").ok();
            }
            return 1;
        }
    }
    1
}
