//! nohup(1) — Run COMMAND ignoring hangup signals.
//!
//! 依據 man page（docs/man/nohup.1.gz）的 EXTRA 段：
//! - stdin 是 terminal → 用 /dev/null 取代
//! - stdout 是 terminal → 改追加到 `nohup.out`；開啟失敗則用 `$HOME/nohup.out`
//! - stderr 是 terminal → 導向 stdout（即同一個檔）
//! 訊息措辭依參考實測（guest 內沒有 terminal，故 terminal 分支由 host 探測取得，
//! 記錄於 docs/assumptions.md）。

use crate::exec::{is_executable, is_tty, resolve_program, status_code, SIG_IGN};
use crate::pathname::resolve_long;
use crate::{In, Out};
use std::os::unix::fs::OpenOptionsExt;
use std::os::unix::process::CommandExt;

const NOHUP_HELP: &str = r#"Run COMMAND ignoring hangup signals.

Usage: nohup COMMAND [ARG]...
       nohup OPTION

Options:
  -h, --help     Print help
  -V, --version  Print version

If standard input is terminal, it'll be replaced with /dev/null.
If standard output is terminal, it'll be appended to nohup.out instead,
or $HOME/nohup.out, if nohup.out open failed.
If standard error is terminal, it'll be redirected to stdout.
"#;

const NOHUP_USAGE: &str = "nohup COMMAND [ARG]...\n       nohup OPTION";
const NOHUP_LONGS: &[&str] = &["help", "version"];

fn clap_tail(stderr: Out) {
    write!(stderr, "\nUsage: {}\n\nFor more information, try '--help'.\n", NOHUP_USAGE).ok();
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

fn missing_command(stderr: Out) -> i32 {
    write!(stderr, "error: the following required arguments were not provided:\n  <cmd>...\n").ok();
    clap_tail(stderr);
    125
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut command: Option<String> = None;
    let mut cmd_args: Vec<String> = Vec::new();
    let mut end_of_options = false;
    let mut i = 0;
    while i < args.len() {
        let a = args[i].clone();
        i += 1;
        if command.is_some() {
            cmd_args.push(a);
            continue;
        }
        if a == "--" {
            end_of_options = true;
            continue;
        }
        if end_of_options {
            command = Some(a);
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let inline = a[1..].find('=').map(|p| a[p + 2..].to_string());
            match resolve_long(&name, NOHUP_LONGS) {
                Ok("help") => {
                    if let Some(v) = inline {
                        return crate::pathname::clap_unexpected_value("help", &v, NOHUP_USAGE, stderr);
                    }
                    write!(stdout, "{}", NOHUP_HELP).ok();
                    return 0;
                }
                Ok("version") => {
                    if let Some(v) = inline {
                        return crate::pathname::clap_unexpected_value("version", &v, NOHUP_USAGE, stderr);
                    }
                    write!(stdout, "nohup (uutils coreutils) 0.10.0\n").ok();
                    return 0;
                }
                Ok(_) => {}
                Err(sim) => return unexpected(&a, sim, stderr),
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let mut bad: Option<char> = None;
            for (k, c) in a[1..].char_indices() {
                match c {
                    'h' => {
                        let _ = k;
                        write!(stdout, "{}", NOHUP_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        write!(stdout, "nohup (uutils coreutils) 0.10.0\n").ok();
                        return 0;
                    }
                    _ => {
                        bad = Some(c);
                        break;
                    }
                }
            }
            if let Some(c) = bad {
                return unexpected(&format!("-{}", c), None, stderr);
            }
            continue;
        }
        command = Some(a);
    }

    let cmd = match command {
        Some(c) => c,
        None => return missing_command(stderr),
    };
    let prog = match resolve_program(&cmd) {
        Some(p) => p,
        None => {
            write!(stderr, "nohup: failed to run command '{}': No such file or directory\n", cmd).ok();
            return 127;
        }
    };
    if !is_executable(&prog) {
        write!(stderr, "nohup: failed to run command '{}': Permission denied\n", cmd).ok();
        return 126;
    }

    let stdin_tty = is_tty(0);
    let stdout_tty = is_tty(1);
    let stderr_tty = is_tty(2);

    let mut builder = std::process::Command::new(&prog);
    builder.args(&cmd_args);
    let mut note: Option<String> = None;
    let mut out_file: Option<std::fs::File> = None;

    if stdin_tty {
        match std::fs::File::open("/dev/null") {
            Ok(devnull) => {
                builder.stdin(std::process::Stdio::from(devnull));
            }
            Err(_) => {}
        }
    }
    if stdout_tty {
        let mut opened: Option<(std::fs::File, String)> = None;
        match std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open("nohup.out") {
            Ok(f) => opened = Some((f, "nohup.out".to_string())),
            Err(_) => {
                if let Ok(home) = std::env::var("HOME") {
                    let p = std::path::Path::new(&home).join("nohup.out");
                    if let Ok(f) = std::fs::OpenOptions::new().create(true).append(true).mode(0o600).open(&p) {
                        opened = Some((f, p.to_string_lossy().into_owned()));
                    }
                }
            }
        }
        if let Some((f, path)) = opened {
            if let Ok(clone) = f.try_clone() {
                builder.stdout(std::process::Stdio::from(clone));
            }
            if stderr_tty {
                if let Ok(clone) = f.try_clone() {
                    builder.stderr(std::process::Stdio::from(clone));
                }
            }
            note = Some(path);
            out_file = Some(f);
        }
    }
    let _ = out_file;

    match note {
        Some(path) => {
            if stdin_tty {
                write!(stderr, "nohup: ignoring input and appending output to '{}'\n", path).ok();
            } else {
                write!(stderr, "nohup: appending output to '{}'\n", path).ok();
            }
        }
        None => {
            if stdin_tty {
                write!(stderr, "nohup: ignoring input\n").ok();
            }
        }
    }

    unsafe {
        builder.pre_exec(|| {
            crate::exec::c_signal(1, SIG_IGN); // SIGHUP
            Ok(())
        });
    }

    match builder.spawn() {
        Ok(mut child) => match child.wait() {
            Ok(status) => status_code(status),
            Err(_) => 126,
        },
        Err(e) => {
            let reason = crate::exec::errno_reason(&e);
            write!(stderr, "nohup: failed to run command '{}': {}\n", cmd, reason).ok();
            126
        }
    }
}
