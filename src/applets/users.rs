use crate::utmp;
use crate::{In, Out};

const USERS_HELP: &str = r#"Print the user names of users currently logged in to the current host.

Usage: users [FILE]

Arguments:
  [file]  

Options:
  -h, --help     Print help
  -V, --version  Print version
"#;

fn unexpected(arg: &str, tip: bool, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found\n\n", arg).ok();
    if tip {
        write!(stderr, "  tip: to pass '{}' as a value, use '-- {}'\n\n", arg, arg).ok();
    }
    write!(
        stderr,
        "Usage: users [FILE]\n\nFor more information, try '--help'.\n"
    )
    .ok();
    1
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut file: Option<String> = None;
    let mut only_positional = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if only_positional {
            if file.is_none() {
                file = Some(a.clone());
                continue;
            }
            return unexpected(a, false, stderr);
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if a == "--help" || a == "-h" || a == "-hV" {
            write!(stdout, "{}", USERS_HELP).ok();
            return 0;
        }
        if a == "--version" || a == "-V" || a == "-Vh" {
            writeln!(stdout, "users (uutils coreutils) 0.10.0").ok();
            return 0;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            return match name.as_str() {
                "help" => {
                    write!(stdout, "{}", USERS_HELP).ok();
                    0
                }
                "version" => {
                    writeln!(stdout, "users (uutils coreutils) 0.10.0").ok();
                    0
                }
                _ => unexpected(a, true, stderr),
            };
        }
        if a.starts_with('-') && a.len() > 1 {
            for k in 1..a.len() {
                match a.as_bytes()[k] {
                    b'h' => {
                        write!(stdout, "{}", USERS_HELP).ok();
                        return 0;
                    }
                    b'V' => {
                        writeln!(stdout, "users (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => return unexpected(&format!("-{}", &a[k..k + 1]), true, stderr),
                }
            }
            continue;
        }
        if file.is_some() {
            return unexpected(a, false, stderr);
        }
        file = Some(a.clone());
    }

    let path = file.unwrap_or_else(|| utmp::DEFAULT_PATH.to_string());
    let mut names: Vec<String> = utmp::read_records(&path)
        .into_iter()
        .filter(|r| r.utype == utmp::USER_PROCESS)
        .map(|r| r.user.clone())
        .collect();
    names.sort();
    if names.is_empty() {
        return 0;
    }
    writeln!(stdout, "{}", names.join(" ")).ok();
    0
}
