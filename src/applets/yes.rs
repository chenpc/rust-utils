//! yes(1) — Repeatedly display a line with STRING (or 'y').
//!
//! 依據 man page（docs/man/yes.1.gz）：
//! - 反覆輸出 STRING（預設 `y`）加 newline
//! - `-h`/`--help`、`-V`/`--version`
//! - 沒有定義「寫入失敗時」的 exit code；這裡假設寫入錯誤回傳 1（見 docs/assumptions.md）。

use crate::{In, Out};

pub fn run(args: &[String], _stdin: In, stdout: Out, _stderr: Out) -> i32 {
    for a in args {
        if a == "-h" || a == "--help" {
            writeln!(stdout, "Usage: yes [-h|--help] [-V|--version] [STRING]").ok();
            return 0;
        }
        if a == "-V" || a == "--version" {
            writeln!(stdout, "yes (rust-coreutils) 0.1.0").ok();
            return 0;
        }
    }

    let line = if args.is_empty() {
        "y".to_string()
    } else {
        args.join(" ")
    };

    loop {
        if writeln!(stdout, "{line}").is_err() {
            return 1;
        }
    }
}
