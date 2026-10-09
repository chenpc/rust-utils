//! true(1) — do nothing, successfully.
//!
//! 依據 man page（docs/man/true.1.gz）：exit status 0（success）。
//! `--help` / `--version` 會印文字後離開；其他參數被忽略。

use crate::{In, Out};

pub fn run(args: &[String], _stdin: In, stdout: Out, _stderr: Out) -> i32 {
    for a in args {
        if a == "--help" {
            writeln!(stdout, "Usage: true [ignored command line arguments] [OPTION]").ok();
            return 0;
        }
        if a == "--version" {
            writeln!(stdout, "true (rust-coreutils) 0.1.0").ok();
            return 0;
        }
    }
    0
}
