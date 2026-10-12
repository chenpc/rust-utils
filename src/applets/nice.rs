//! nice(1) — Run COMMAND with an adjusted niceness.
//!
//! 依據 man page（docs/man/nice.1.gz）：無 COMMAND 時印出目前 niceness；
//! 有 COMMAND 時把 adjustment（預設 10）加上去再執行。
//! 措辭與 exit code 依本機參考實測（見 docs/assumptions.md）；
//! 差異：參考用 exec 取代行程，本專案改用 spawn + wait（測試為 in-process）。

use crate::exec::{
    c_setpriority, current_niceness, errno_reason, is_executable, resolve_program, status_code,
    PRIO_PROCESS,
};
use crate::pathname::{clap_value_required, resolve_long, usage_hint};
use crate::{In, Out};

const NICE_HELP: &str = r#"Run COMMAND with an adjusted niceness, which affects process scheduling.
With no COMMAND, print the current niceness.  Niceness values range from
-20 (most favorable to the process) to 19 (least favorable to the process).

Usage: nice [OPTION] [COMMAND [ARG]...]

Arguments:
  [COMMAND]...  

Options:
  -n, --adjustment <adjustment>  add N to the niceness (default is 10)
  -h, --help                     Print help
  -V, --version                  Print version
"#;

const NICE_USAGE: &str = "nice [OPTION] [COMMAND [ARG]...]";
const NICE_LONGS: &[&str] = &["adjustment", "help", "version"];

fn clap_tail(stderr: Out) {
    write!(stderr, "\nUsage: {}\n\nFor more information, try '--help'.\n", NICE_USAGE).ok();
}

fn unexpected(arg: &str, similar: Option<&str>, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found\n\n", arg).ok();
    if let Some(c) = similar {
        writeln!(stderr, "tip: a similar argument exists: '--{}'", c).ok();
    } else {
        writeln!(stderr, "  tip: to pass '{}' as a value, use '-- {}'", arg, arg).ok();
    }
    clap_tail(stderr);
    125
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut adjustment: Option<String> = None;
    let mut command: Option<String> = None;
    let mut cmd_args: Vec<String> = Vec::new();
    let mut end_of_options = false;
    let mut i = 0usize;

    while i < args.len() {
        let a = args[i].clone();
        i += 1;
        if end_of_options || command.is_some() {
            cmd_args.push(a);
            continue;
        }
        if a == "--" {
            end_of_options = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let inline = a[1..].find('=').map(|p| a[p + 2..].to_string());
            match resolve_long(&name, NICE_LONGS) {
                Ok("help") => {
                    if let Some(v) = inline {
                        return crate::pathname::clap_unexpected_value("help", &v, NICE_USAGE, stderr);
                    }
                    write!(stdout, "{}", NICE_HELP).ok();
                    return 0;
                }
                Ok("version") => {
                    if let Some(v) = inline {
                        return crate::pathname::clap_unexpected_value("version", &v, NICE_USAGE, stderr);
                    }
                    write!(stdout, "nice (uutils coreutils) 0.10.0\n").ok();
                    return 0;
                }
                Ok("adjustment") => {
                    if let Some(v) = inline {
                        adjustment = Some(v);
                    } else if i < args.len() {
                        adjustment = Some(args[i].clone());
                        i += 1;
                    } else {
                        clap_value_required("adjustment", "adjustment", stderr);
                            return 125;
                    }
                }
                Ok(_) => {}
                Err(sim) => return unexpected(&a, sim, stderr),
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let rest = &a[1..];
            // 舊式 `-10`：整個 token 就是 adjustment。
            if !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit() || c == '-') {
                adjustment = Some(a.clone());
                continue;
            }
            let chars: Vec<char> = rest.chars().collect();
            let mut k = 0usize;
            while k < chars.len() {
                match chars[k] {
                    'h' => {
                        write!(stdout, "{}", NICE_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        write!(stdout, "nice (uutils coreutils) 0.10.0\n").ok();
                        return 0;
                    }
                    'n' => {
                        k += 1;
                        if k < chars.len() {
                            adjustment = Some(chars[k..].iter().collect());
                            break;
                        } else if i < args.len() {
                            adjustment = Some(args[i].clone());
                            i += 1;
                            break;
                        } else {
                            clap_value_required("adjustment", "adjustment", stderr);
                            return 125;
                        }
                    }
                    c => return unexpected(&format!("-{}", c), None, stderr),
                }
            }
            continue;
        }
        command = Some(a);
    }

    let cmd = match command {
        Some(c) => c,
        None => {
            if adjustment.is_some() {
                write!(stderr, "nice: A command must be given with an adjustment.\n").ok();
                write!(stderr, "{}\n", usage_hint("nice")).ok();
                return 125;
            }
            writeln!(stdout, "{}", current_niceness()).ok();
            return 0;
        }
    };

    let raw_adj = adjustment.clone().unwrap_or_else(|| "10".to_string());
    let adj = match raw_adj.parse::<i32>() {
        Ok(v) => v,
        Err(_) => {
            write!(
                stderr,
                "nice: \"{}\" is not a valid number: invalid digit found in string\n",
                raw_adj
            )
            .ok();
            return 125;
        }
    };

    let target = current_niceness() + adj;
    unsafe {
        if c_setpriority(PRIO_PROCESS, 0, target) == -1 {
            let reason = errno_reason(&std::io::Error::last_os_error());
            write!(stderr, "nice: warning: setpriority: {}\n", reason).ok();
        }
    }

    let prog = match resolve_program(&cmd) {
        Some(p) => p,
        None => {
            write!(
                stderr,
                "nice: {}: No such file or directory (os error 2)\n",
                cmd
            )
            .ok();
            return 127;
        }
    };
    if !is_executable(&prog) {
        write!(stderr, "nice: {}: Permission denied (os error 13)\n", cmd).ok();
        return 126;
    }

    let mut builder = std::process::Command::new(&prog);
    builder.args(&cmd_args);
    match builder.spawn() {
        Ok(mut child) => match child.wait() {
            Ok(status) => status_code(status),
            Err(_) => 126,
        },
        Err(e) => {
            write!(stderr, "nice: {}: {}\n", cmd, errno_reason(&e)).ok();
            126
        }
    }
}
