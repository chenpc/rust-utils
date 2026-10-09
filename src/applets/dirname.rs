//! dirname(1) — Strip last component from file name.
//!
//! 依據 man page（docs/man/dirname.1.gz）+ GNU coreutils 9.5 info：
//! - `-z|--zero`、`-h`、`-V`
//! - NAME 不含 `/` → `.`；NAME 全是 `/` → `/`（Linux）；NAME 為空 → `.`
//! - 多個 NAME 一律各自處理（man page SYNOPSIS 只列 [dir]，GNU 支援多 operand）

use crate::pathname as p;
use crate::{In, Out};

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut zero = false;
    let mut operands: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    for a in args {
        if no_more_opts || a == "-" || !a.starts_with('-') || a == "-" {
            operands.push(a.clone());
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            match &a[2..] {
                "zero" => zero = true,
                "help" => {
                    writeln!(stdout, "Usage: dirname [-z] [-h|--help] [-V|--version] [dir]").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "dirname (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                name => {
                    writeln!(stderr, "dirname: unrecognized option '--{name}'").ok();
                    return 2;
                }
            }
            continue;
        }
        for c in a[1..].chars() {
            match c {
                'z' => zero = true,
                'h' => {
                    writeln!(stdout, "Usage: dirname [-z] [-h|--help] [-V|--version] [dir]").ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "dirname (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => {
                    writeln!(stderr, "dirname: invalid option -- '{c}'").ok();
                    return 2;
                }
            }
        }
    }
    if operands.is_empty() {
        writeln!(stderr, "dirname: missing operand").ok();
        return 1;
    }
    let mut status = 0;
    for name in &operands {
        let d = p::dir_part(name);
        let wrote = if zero {
            stdout
                .write_all(d.as_bytes())
                .and_then(|_| stdout.write_all(&[0u8]))
        } else {
            writeln!(stdout, "{d}")
        };
        if wrote.is_err() {
            status = 1;
        }
    }
    status
}
