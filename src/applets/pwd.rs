//! pwd(1) — Display the full filename of the current working directory.
//!
//! 依據 man page（docs/man/pwd.1.gz）：
//! `  -L, --logical   use PWD from environment, even if it contains symlinks`
//! `  -P, --physical  avoid all symlinks`
//! man 未載明預設採用哪一種 → 依本機參考實測採 physical（見 docs/assumptions.md）。
//! 兩個選項同時出現時「後出現者贏」（實測：`-P -L` → logical、`-L -P` → physical）。
//! 非選項 operand 被忽略但印 `pwd: ignoring non-option arguments` 到 stderr（rc0）。

use crate::pathname::infer_long_opt;
use crate::{In, Out};
use std::os::unix::fs::MetadataExt;

const PWD_HELP: &str = r#"Display the full filename of the current working directory.

Usage: pwd [OPTION]...

Options:
  -L, --logical   use PWD from environment, even if it contains symlinks
  -P, --physical  avoid all symlinks
  -h, --help      Print help
  -V, --version   Print version"#;

const PWD_LONGS: &[&str] = &["help", "logical", "physical", "version"];

fn clap_unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "  tip: to pass '{}' as a value, use '-- {}'", arg, arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "Usage: pwd [OPTION]...").ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}

fn current_physical() -> Result<String, std::io::Error> {
    let cwd = std::env::current_dir()?;
    Ok(cwd.to_string_lossy().into_owned())
}

/// -L：PWD 若為「指向目前 cwd 的絕對路徑」就直接印（可含連字號），否則退回 physical。
fn logical_display(physical: &str) -> String {
    if let Ok(pwd) = std::env::var("PWD") {
        let p = std::path::Path::new(&pwd);
        if !pwd.is_empty() && p.is_absolute() {
            if let (Ok(a), Ok(b)) = (
                std::fs::metadata(p),
                std::env::current_dir().and_then(|d| std::fs::metadata(d)),
            ) {
                if a.dev() == b.dev() && a.ino() == b.ino() {
                    return pwd;
                }
            }
        }
    }
    physical.to_string()
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut logical = false;
    let mut logical_given = false;
    let mut physical_given = false;
    let mut ignored = false;
    let mut no_more_opts = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        i += 1;
        if no_more_opts {
            ignored = true;
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            match infer_long_opt(&name, PWD_LONGS) {
                Some("help") => {
                    writeln!(stdout, "{}", PWD_HELP).ok();
                    return 0;
                }
                Some("version") => {
                    writeln!(stdout, "pwd (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                Some("logical") => {
                    if logical_given {
                        return crate::pathname::clap_dup("logical", "", 1, stderr);
                    }
                    logical_given = true;
                    logical = true;
                }
                Some("physical") => {
                    if physical_given {
                        return crate::pathname::clap_dup("physical", "", 1, stderr);
                    }
                    physical_given = true;
                    logical = false;
                }
                _ => return clap_unexpected(&a, stderr),
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            for c in a[1..].chars() {
                match c {
                    'L' => {
                        if logical_given {
                            return crate::pathname::clap_dup("logical", "", 1, stderr);
                        }
                        logical_given = true;
                        logical = true;
                    }
                    'P' => {
                        if physical_given {
                            return crate::pathname::clap_dup("physical", "", 1, stderr);
                        }
                        physical_given = true;
                        logical = false;
                    }
                    'h' => {
                        writeln!(stdout, "{}", PWD_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        writeln!(stdout, "pwd (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => return clap_unexpected(&format!("-{}", c), stderr),
                }
            }
            continue;
        }
        ignored = true;
    }

    let physical = match current_physical() {
        Ok(p) => p,
        Err(e) => {
            writeln!(stderr, "pwd: cannot get current directory: {}", e).ok();
            return 1;
        }
    };
    let out = if logical {
        logical_display(&physical)
    } else {
        physical
    };
    writeln!(stdout, "{}", out).ok();
    if ignored {
        writeln!(stderr, "pwd: ignoring non-option arguments").ok();
    }
    0
}
