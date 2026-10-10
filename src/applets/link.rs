//! link(1) — Call the link system call.
//!
//! 措辭依參考：`link: cannot create link 'l1' to 'src1': Already exists`。
//! 僅接受 FILE1 FILE2 兩個運算元（參考 clap 措辭不追隨，見 assumptions）。

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
                writeln!(stdout, "Usage: link FILE1 FILE2\nCall the link function to create a link named FILE2 to an existing FILE1.").ok();
                return 0;
            }
            "--version" | "-V" => {
                writeln!(stdout, "link (uutils coreutils) 0.10.0").ok();
                return 0;
            }
            _ => {
                writeln!(stderr, "link: unrecognized option '{}'", a).ok();
                return 2;
            }
        }
    }
    if operands.len() != 2 {
        writeln!(stderr, "link: missing file operand").ok();
        return 2;
    }
    let (target, linkname) = (operands[0].clone(), operands[1].clone());
    if let Err(e) = std::fs::hard_link(&target, &linkname) {
        let reason = if e.raw_os_error() == Some(17) { "Already exists".to_string() } else { crate::pathname::io_reason_nosuffix(&e) };
        writeln!(stderr, "link: cannot create link '{}' to '{}': {}", linkname, target, reason).ok();
        return 1;
    }
    0
}
