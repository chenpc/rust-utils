//! tty — 印出連到 standard input 的終端機檔名。
//!
//! 規格來源：`docs/man/tty.1.gz`（OPTIONS：`-s, --silent`、`-h`、`-V`）。
//! man 沒寫「是不是 tty」的輸出措辭，也沒寫NAME 欄是否附換行；
//! 依本機參考實測：非 tty → `not a tty\n`（rc1）；是 tty → 檔名**不附**換行（rc0）。

use crate::pathname;
use crate::sys;
use crate::{In, Out};

const TTY_HELP: &str = r#"Print the file name of the terminal connected to standard input.

Usage: tty [OPTION]...

Options:
  -s, --silent   print nothing, only return an exit status [alias: --quiet]
  -h, --help     Print help
  -V, --version  Print version
"#;

const TTY_USAGE: &str = "tty [OPTION]...";
const TTY_LONGS: &[&str] = &["help", "quiet", "silent", "version"];

fn unexpected(arg: &str, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found\n\n", arg).ok();
    write!(
        stderr,
        "Usage: {}\n\nFor more information, try '--help'.\n",
        TTY_USAGE
    )
    .ok();
    2
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut silent = false;
    let mut only_positional = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if only_positional {
            return unexpected(a, stderr);
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let value = a.split_once('=').map(|(_, v)| v.to_string());
            match pathname::resolve_long(&name, TTY_LONGS) {
                Ok("help") => {
                    if let Some(v) = value {
                        pathname::clap_unexpected_value("help", &v, TTY_USAGE, stderr);
                        return 2;
                    }
                    write!(stdout, "{}", TTY_HELP).ok();
                    return 0;
                }
                Ok("version") => {
                    if let Some(v) = value {
                        pathname::clap_unexpected_value("version", &v, TTY_USAGE, stderr);
                        return 2;
                    }
                    writeln!(stdout, "tty (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                Ok("silent") | Ok("quiet") => {
                    if let Some(v) = value {
                        pathname::clap_unexpected_value("silent", &v, TTY_USAGE, stderr);
                        return 2;
                    }
                    if silent {
                        return pathname::clap_dup("silent", "", 2, stderr);
                    }
                    silent = true;
                }
                Ok(other) => return unexpected(&format!("--{}", other), stderr),
                Err(similar) => {
                    write!(stderr, "error: unexpected argument '{}' found\n\n", a).ok();
                    if let Some(c) = similar {
                        write!(stderr, "tip: a similar argument exists: '--{}'\n\n", c).ok();
                    }
                    write!(
                        stderr,
                        "Usage: {}\n\nFor more information, try '--help'.\n",
                        TTY_USAGE
                    )
                    .ok();
                    return 2;
                }
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let mut k = 1;
            while k < a.len() {
                match a.as_bytes()[k] {
                    b's' => {
                        if silent {
                            return pathname::clap_dup("silent", "", 2, stderr);
                        }
                        silent = true;
                    }
                    b'h' => {
                        write!(stdout, "{}", TTY_HELP).ok();
                        return 0;
                    }
                    b'V' => {
                        writeln!(stdout, "tty (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => return unexpected(&pathname::clap_unknown_short(a, k), stderr),
                }
                k += 1;
            }
            continue;
        }
        return unexpected(a, stderr);
    }
    match sys::tty_name() {
        Some(name) => {
            if !silent {
                write!(stdout, "{}", name).ok();
            }
            0
        }
        None => {
            if !silent {
                writeln!(stdout, "not a tty").ok();
            }
            1
        }
    }
}
