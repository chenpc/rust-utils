//! unlink(1) — Call the unlink system call.
//!
//! 措辭依參考：`unlink: cannot unlink 'l1': No such file or directory`。

use crate::{In, Out};

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut operands: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    for a in args {
        if no_more_opts || (!a.starts_with('-') || a == "-") {
            operands.push(a.clone());
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        match a.as_str() {
            "--help" | "-h" => {
                writeln!(stdout, "Usage: unlink FILE\nCall the unlink function to remove the specified FILE.").ok();
                return 0;
            }
            "--version" | "-V" => {
                writeln!(stdout, "unlink (uutils coreutils) 0.10.0").ok();
                return 0;
            }
            _ => {
                writeln!(stderr, "unlink: unrecognized option '{}'", a).ok();
                return 2;
            }
        }
    }
    if operands.len() != 1 {
        writeln!(stderr, "unlink: missing operand").ok();
        return 2;
    }
    let f = &operands[0];
    if let Err(e) = std::fs::remove_file(f) {
        writeln!(stderr, "unlink: cannot unlink '{}': {}", f, crate::pathname::io_reason_pub(&e)).ok();
        return 1;
    }
    0
}
