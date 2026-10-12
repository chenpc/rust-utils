use crate::sys;
use crate::{In, Out};

const UNAME_HELP: &str = r#"Print certain system information.
With no OPTION, same as -s.

Usage: uname [OPTION]...

Options:
  -a, --all               Behave as though all of the options -mnrsvo were specified.
  -s, --kernel-name       print the kernel name.
  -n, --nodename          print the nodename (the nodename may be a name that the system
                          is known by to a communications network).
  -r, --kernel-release    print the operating system release.
  -v, --kernel-version    print the operating system version.
  -m, --machine           print the machine hardware name.
  -o, --operating-system  print the operating system name.
  -h, --help              Print help
  -V, --version           Print version
"#;

fn unexpected(arg: &str, stderr: Out) -> i32 {
    write!(stderr, 
        "error: unexpected argument '{}' found\n\nUsage: uname [OPTION]...\n\nFor more information, try '--help'.\n",
        arg
    ).ok();
    1
}

fn dup(name: &str, stderr: Out) -> i32 {
    write!(stderr, 
        "error: the argument '{}' cannot be used multiple times\n\nFor more information, try '--help'.\n",
        name
    ).ok();
    1
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut all = false;
    let mut parts: Vec<char> = Vec::new();
    let mut end_of_options = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if end_of_options {
            return unexpected(a, stderr);
        }
        if a == "--" {
            end_of_options = true;
            continue;
        }
        if a == "--help" || a == "-h" || a == "-hV" {
            write!(stdout, "{}", UNAME_HELP).ok();
            return 0;
        }
        if a == "--version" || a == "-V" || a == "-Vh" {
            writeln!(stdout, "uname (uutils coreutils) 0.10.0").ok();
            return 0;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let flag = match name.as_str() {
                "all" => Some('a'),
                "kernel-name" => Some('s'),
                "nodename" => Some('n'),
                "kernel-release" => Some('r'),
                "kernel-version" => Some('v'),
                "machine" => Some('m'),
                "operating-system" => Some('o'),
                "help" => {
                    write!(stdout, "{}", UNAME_HELP).ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "uname (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => return unexpected(a, stderr),
            };
            let flag = flag.unwrap();
            if flag == 'a' {
                if all {
                    return dup("--all", stderr);
                }
                all = true;
            } else if parts.contains(&flag) {
                return dup(long_name(flag), stderr);
            } else {
                parts.push(flag);
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            for k in 1..a.len() {
                let c = a.as_bytes()[k] as char;
                match c {
                    'a' => {
                        if all {
                            return dup("--all", stderr);
                        }
                        all = true;
                    }
                    's' | 'n' | 'r' | 'v' | 'm' | 'o' => {
                        if parts.contains(&c) {
                            return dup(long_name(c), stderr);
                        }
                        parts.push(c);
                    }
                    'h' => {
                        write!(stdout, "{}", UNAME_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        writeln!(stdout, "uname (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => return unexpected(&format!("-{}", c), stderr),
                }
            }
            continue;
        }
        return unexpected(a, stderr);
    }

    if all {
        parts = vec!['s', 'n', 'r', 'v', 'm', 'o'];
    }
    if parts.is_empty() {
        parts.push('s');
    }
    // Output follows the canonical order regardless of the order the flags were given.
    let order = ['s', 'n', 'r', 'v', 'm', 'o'];
    parts.sort_by_key(|c| order.iter().position(|x| *x == *c).unwrap());
    let u = match sys::utsname() {
        Some(u) => u,
        None => {
            writeln!(stderr, "uname: cannot get system information").ok();
            return 1;
        }
    };
    let mut out: Vec<String> = Vec::new();
    for c in parts {
        let value = match c {
            's' => sys::field_str(&u.sysname),
            'n' => sys::field_str(&u.nodename),
            'r' => sys::field_str(&u.release),
            'v' => sys::field_str(&u.version),
            'm' => sys::field_str(&u.machine),
            _ => "GNU/Linux".to_string(),
        };
        out.push(value);
    }
    writeln!(stdout, "{}", out.join(" ")).ok();
    0
}

fn long_name(c: char) -> &'static str {
    match c {
        's' => "--kernel-name",
        'n' => "--nodename",
        'r' => "--kernel-release",
        'v' => "--kernel-version",
        'm' => "--machine",
        _ => "--operating-system",
    }
}
