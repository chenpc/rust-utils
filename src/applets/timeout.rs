//! timeout(1) — Start COMMAND, and kill it if still running after DURATION.
//!
//! 依據 man page（docs/man/timeout.1.gz）與本機參考實測：
//! - 預設把 COMMAND 放在自己的 process group，逾時時對整個 group 發訊號；
//!   `-f/--foreground` 則只對 COMMAND 本身發訊號。
//! - 逾時 → exit 124；`-p/--preserve-status` → 128+訊號號；`-k` 升級到 KILL → 137。
//! - 找不到指令 → 127；找到但不可執行 → 126。
//! 差異：參考用 exec，本專案改用 spawn + wait（測試為 in-process）。

use crate::exec::{
    c_kill, is_executable, parse_duration, resolve_program, signal_name, signal_number, status_code,
};
use crate::pathname::{clap_value_required, resolve_long, usage_hint};
use crate::{In, Out};
use std::os::unix::process::CommandExt;
use std::time::{Duration, Instant};

const TIMEOUT_HELP: &str = r#"Start COMMAND, and kill it if still running after DURATION.

Usage: timeout [OPTION] DURATION COMMAND...

Arguments:
  <duration>    a floating point number with an optional suffix: 's' for seconds (the default), 'm'
                for minutes, 'h' for hours or 'd' for days ; a duration of 0 disables the associated
                timeout
  <command>...  a command to execute with optional arguments

Options:
  -f, --foreground               when not running timeout directly from a shell prompt, allow
                                 COMMAND to read from the TTY and get TTY signals; in this mode,
                                 children of COMMAND will not be timed out
  -k, --kill-after <kill-after>  also send a KILL signal if COMMAND is still running this long after
                                 the initial signal was sent
  -p, --preserve-status          exit with the same status as COMMAND, even when the command times
                                 out
  -s, --signal <SIGNAL>          specify the signal to be sent on timeout; SIGNAL may be a name like
                                 'HUP' or a number; see 'kill -l' for a list of signals
  -v, --verbose                  diagnose to stderr any signal sent upon timeout
  -h, --help                     Print help
  -V, --version                  Print version

Upon timeout, send the TERM signal to COMMAND, if no other SIGNAL specified. The TERM signal kills
any process that does not block or catch that signal. It may be necessary to use the KILL signal,
since this signal can't be caught.

Exit status:
  124  if COMMAND times out, and --preserve-status is not specified
  125  if the timeout command itself fails
  126  if COMMAND is found but cannot be invoked
  127  if COMMAND cannot be found
  137  if COMMAND (or timeout itself) is sent the KILL (9) signal (128+9)
  -    the exit status of COMMAND otherwise
"#;

const TIMEOUT_USAGE: &str = "timeout [OPTION] DURATION COMMAND...";
const TIMEOUT_LONGS: &[&str] = &[
    "foreground",
    "kill-after",
    "preserve-status",
    "signal",
    "verbose",
    "help",
    "version",
];

fn clap_tail(stderr: Out) {
    write!(
        stderr,
        "\nUsage: {}\n\nFor more information, try '--help'.\n",
        TIMEOUT_USAGE
    )
    .ok();
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

fn missing(stderr: Out, labels: &[&str]) -> i32 {
    write!(
        stderr,
        "error: the following required arguments were not provided:\n"
    )
    .ok();
    for label in labels {
        writeln!(stderr, "  {}", label).ok();
    }
    clap_tail(stderr);
    125
}

pub fn run(args: &[String], _stdin: In, _stdout: Out, stderr: Out) -> i32 {
    let mut foreground = false;
    let mut preserve = false;
    let mut verbose = 0usize;
    let mut kill_after_spec: Option<String> = None;
    let mut signal_spec: Option<String> = None;
    let mut duration_spec: Option<String> = None;
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
            match resolve_long(&name, TIMEOUT_LONGS) {
                Ok("help") => {
                    if let Some(v) = inline {
                        return crate::pathname::clap_unexpected_value("help", &v, TIMEOUT_USAGE, stderr);
                    }
                    write!(_stdout, "{}", TIMEOUT_HELP).ok();
                    return 0;
                }
                Ok("version") => {
                    if let Some(v) = inline {
                        return crate::pathname::clap_unexpected_value("version", &v, TIMEOUT_USAGE, stderr);
                    }
                    write!(_stdout, "timeout (uutils coreutils) 0.10.0\n").ok();
                    return 0;
                }
                Ok("foreground") => foreground = true,
                Ok("preserve-status") => preserve = true,
                Ok("verbose") => verbose += 1,
                Ok("kill-after") => {
                    kill_after_spec = Some(match inline {
                        Some(v) => v,
                        None => {
                            if i < args.len() {
                                let v = args[i].clone();
                                i += 1;
                                v
                            } else {
                                clap_value_required("kill-after", "kill-after", stderr);
                                return 125;
                            }
                        }
                    })
                }
                Ok("signal") => {
                    signal_spec = Some(match inline {
                        Some(v) => v,
                        None => {
                            if i < args.len() {
                                let v = args[i].clone();
                                i += 1;
                                v
                            } else {
                                clap_value_required("signal", "SIGNAL", stderr);
                                return 125;
                            }
                        }
                    })
                }
                Ok(_) => {}
                Err(sim) => return unexpected(&a, sim, stderr),
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let chars: Vec<char> = a[1..].chars().collect();
            let mut k = 0usize;
            while k < chars.len() {
                match chars[k] {
                    'f' => foreground = true,
                    'p' => preserve = true,
                    'v' => verbose += 1,
                    'h' => {
                        write!(_stdout, "{}", TIMEOUT_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        write!(_stdout, "timeout (uutils coreutils) 0.10.0\n").ok();
                        return 0;
                    }
                    'k' | 's' => {
                        k += 1;
                        let val = if k < chars.len() {
                            chars[k..].iter().collect()
                        } else if i < args.len() {
                            let v = args[i].clone();
                            i += 1;
                            v
                        } else {
                            let long = if chars[k - 1] == 'k' { "kill-after" } else { "signal" };
                            let metavar = if long == "signal" { "SIGNAL" } else { "kill-after" };
                            clap_value_required(long, metavar, stderr);
                            return 125;
                        };
                        if chars[k - 1] == 'k' {
                            kill_after_spec = Some(val);
                        } else {
                            signal_spec = Some(val);
                        }
                        break;
                    }
                    c => return unexpected(&format!("-{}", c), None, stderr),
                }
                k += 1;
            }
            continue;
        }
        if duration_spec.is_none() {
            duration_spec = Some(a);
        } else if command.is_none() {
            command = Some(a);
        } else {
            cmd_args.push(a);
        }
    }

    let dur_spec = match duration_spec {
        Some(d) => d,
        None => return missing(stderr, &["<duration>", "<command>..."]),
    };
    let cmd = match command {
        Some(c) => c,
        None => return missing(stderr, &["<command>..."]),
    };

    let duration = match parse_duration(&dur_spec) {
        Some(v) => v,
        None => {
            write!(stderr, "timeout: invalid time interval '{}'\n", dur_spec).ok();
            write!(stderr, "{}\n", usage_hint("timeout")).ok();
            return 125;
        }
    };
    let kill_after = match kill_after_spec.as_deref() {
        Some(spec) => match parse_duration(spec) {
            Some(v) => Some(v),
            None => {
                write!(stderr, "timeout: invalid time interval '{}'\n", spec).ok();
                write!(stderr, "{}\n", usage_hint("timeout")).ok();
                return 125;
            }
        },
        None => None,
    };
    let sig = match signal_spec.as_deref() {
        Some(spec) => match signal_number(spec) {
            Some(s) => s,
            None => {
                write!(stderr, "timeout: '{}': invalid signal\n", spec).ok();
                write!(stderr, "{}\n", usage_hint("timeout")).ok();
                return 125;
            }
        },
        None => 15,
    };

    let prog = match resolve_program(&cmd) {
        Some(p) => p,
        None => {
            write!(
                stderr,
                "timeout: failed to execute process: No such file or directory (os error 2)\n"
            )
            .ok();
            return 127;
        }
    };
    if !is_executable(&prog) {
        write!(
            stderr,
            "timeout: failed to execute process: Permission denied (os error 13)\n"
        )
        .ok();
        return 126;
    }

    let mut builder = std::process::Command::new(&prog);
    builder.args(&cmd_args);
    if !foreground {
        builder.process_group(0);
    }
    let mut child = match builder.spawn() {
        Ok(c) => c,
        Err(e) => {
            write!(
                stderr,
                "timeout: failed to execute process: {}\n",
                crate::exec::errno_reason(&e)
            )
            .ok();
            return 126;
        }
    };

    let start = Instant::now();
    let deadline = if duration > 0.0 {
        Some(start + Duration::from_secs_f64(duration))
    } else {
        None
    };
    let target = if foreground {
        child.id() as i32
    } else {
        -(child.id() as i32)
    };

    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status_code(status),
            Ok(None) => {}
            Err(_) => return 126,
        }
        let now = Instant::now();
        match deadline {
            Some(dl) if now >= dl => {}
            _ => {
                std::thread::sleep(Duration::from_millis(5));
                continue;
            }
        }
        break;
    }

    if verbose > 0 {
        write!(
            stderr,
            "timeout: sending signal {} to command '{}'\n",
            signal_name(sig),
            cmd
        )
        .ok();
    }
    unsafe { c_kill(target, sig) };

    let mut killed_with = sig;
    if let Some(ka) = kill_after {
        let ka_deadline = Instant::now() + Duration::from_secs_f64(ka);
        while Instant::now() < ka_deadline {
            if let Ok(Some(status)) = child.try_wait() {
                return if preserve { 128 + sig } else { status_code_or_timeout(status) };
            }
            std::thread::sleep(Duration::from_millis(5));
        }
        if child.try_wait().map(|s| s.is_none()).unwrap_or(false) {
            if verbose > 0 {
                write!(
                    stderr,
                    "timeout: sending signal KILL to command '{}'\n",
                    cmd
                )
                .ok();
            }
            unsafe { c_kill(target, 9) };
            killed_with = 9;
        }
    }
    let _ = child.wait();
    if preserve {
        128 + killed_with
    } else if killed_with == 9 {
        137
    } else {
        124
    }
}

fn status_code_or_timeout(status: std::process::ExitStatus) -> i32 {
    match status.code() {
        Some(c) => c,
        None => 124,
    }
}
