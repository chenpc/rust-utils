//! rmdir(1) — Remove directories.
//!
//! 措辭依參考（uutils 0.10.0）："rmdir: failed to remove 'b': Directory not empty"；
//! `--ignore-fail-on-non-empty` 只吞 ENOTEMPTY（ENOENT 仍報錯）；
//! `-p` 逐級上删、失敗即停；`-v` 先印 "removing directory, 'X'"。

use crate::{In, Out};
use std::path::PathBuf;

pub fn run(args: &[String], _stdin: In, _stdout: Out, stderr: Out) -> i32 {
    let mut parents = false;
    let mut ignore_nonempty = false;
    let mut verbose = false;
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
        if a.starts_with("--") {
            match &a[2..] {
                "parents" => parents = true,
                "ignore-fail-on-non-empty" => ignore_nonempty = true,
                "verbose" => verbose = true,
                "help" => {
                    writeln!(_stdout, "Usage: rmdir [OPTION]... DIRECTORY...\nRemove the DIRECTORY(ies), if they are empty.").ok();
                    return 0;
                }
                "version" => {
                    writeln!(_stdout, "rmdir (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => {
                    writeln!(stderr, "rmdir: unrecognized option '{}'", a).ok();
                    return 2;
                }
            }
            continue;
        }
        for c in a[1..].chars() {
            match c {
                'p' => parents = true,
                'v' => verbose = true,
                'h' => {
                    writeln!(_stdout, "Usage: rmdir [OPTION]... DIRECTORY...").ok();
                    return 0;
                }
                'V' => {
                    writeln!(_stdout, "rmdir (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => {
                    writeln!(stderr, "rmdir: invalid option -- '{}'", c).ok();
                    return 2;
                }
            }
        }
    }
    if operands.is_empty() {
        writeln!(stderr, "rmdir: missing operand").ok();
        return 2;
    }
    let mut rc = 0;
    for op in &operands {
        let mut chain: Vec<String> = Vec::new();
        if parents {
            let mut cur = op.clone();
            loop {
                chain.push(cur.clone());
                if cur.is_empty() || cur == "/" || cur == "." {
                    break;
                }
                match parent_of(&cur) {
                    Some(p) => cur = p,
                    None => break,
                }
            }
        } else {
            chain.push(op.clone());
        }
        for p in chain {
            if verbose {
                writeln!(stderr, "rmdir: removing directory, '{}'", p).ok();
            }
            match std::fs::remove_dir(&p) {
                Ok(()) => {}
                Err(e) => {
                    if ignore_nonempty && e.raw_os_error() == Some(39 /* ENOTEMPTY */) {
                        break;
                    }
                    writeln!(stderr, "rmdir: failed to remove '{}': {}", p, crate::pathname::io_reason_nosuffix(&e)).ok();
                    rc = 1;
                    break;
                }
            }
        }
    }
    rc
}

fn parent_of(p: &str) -> Option<String> {
    let pb = PathBuf::from(p);
    let parent = pb.parent()?;
    let s = parent.to_string_lossy().to_string();
    if s.is_empty() || s == "." {
        return None; // 到達相對路徑頂層即停
    }
    Some(s)
}
