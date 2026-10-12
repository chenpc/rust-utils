use crate::pathname;
use crate::sys;
use crate::{In, Out};

const ARCH_HELP: &str = r#"Display machine architecture

Usage: arch

Options:
  -h, --help     Print help
  -V, --version  Print version

Determine architecture name for current machine.
"#;

const ARCH_LONGS: &[&str] = &["help", "version"];

fn unexpected(arg: &str, similar: Option<&str>, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found\n\n", arg).ok();
    if let Some(c) = similar {
        write!(stderr, "tip: a similar argument exists: '--{}'\n\n", c).ok();
    }
    write!(
        stderr,
        "Usage: arch\n\nFor more information, try '--help'.\n"
    )
    .ok();
    1
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut only_positional = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if only_positional {
            return unexpected(a, None, stderr);
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if a == "--help" || a == "-h" || a == "-hV" {
            write!(stdout, "{}", ARCH_HELP).ok();
            return 0;
        }
        if a == "--version" || a == "-V" || a == "-Vh" {
            writeln!(stdout, "arch (uutils coreutils) 0.10.0").ok();
            return 0;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let value = a.split_once('=').map(|(_, v)| v.to_string());
            return match pathname::resolve_long(&name, ARCH_LONGS) {
                Ok("help") => match value {
                    Some(v) => pathname::clap_unexpected_value("help", &v, "arch", stderr),
                    None => {
                        write!(stdout, "{}", ARCH_HELP).ok();
                        0
                    }
                },
                Ok("version") => match value {
                    Some(v) => pathname::clap_unexpected_value("version", &v, "arch", stderr),
                    None => {
                        writeln!(stdout, "arch (uutils coreutils) 0.10.0").ok();
                        0
                    }
                },
                Ok(other) => unexpected(&format!("--{}", other), None, stderr),
                Err(similar) => unexpected(a, similar, stderr),
            };
        }
        if a.starts_with('-') && a.len() > 1 {
            for k in 1..a.len() {
                match a.as_bytes()[k] {
                    b'h' => {
                        write!(stdout, "{}", ARCH_HELP).ok();
                        return 0;
                    }
                    b'V' => {
                        writeln!(stdout, "arch (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => {
                        return unexpected(&format!("-{}", &a[k..k + 1]), None, stderr);
                    }
                }
            }
            continue;
        }
        return unexpected(a, None, stderr);
    }
    match sys::machine_name() {
        Some(m) => {
            writeln!(stdout, "{}", m).ok();
            0
        }
        None => {
            writeln!(stderr, "arch: cannot get machine name").ok();
            1
        }
    }
}
