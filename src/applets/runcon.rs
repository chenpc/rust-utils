//! runcon — 在指定的 SELinux security context 執行指令。
//!
//! 規格來源：`docs/man/runcon.1.gz`
//! （OPTIONS：`-c/--compute`、`-u/--user <USER>`、`-r/--role <ROLE>`、`-t/--type <TYPE>`、
//! `-l/--range <RANGE>`、`-h`、`-V`；`[ARG]`；EXTRA：「If neither CONTEXT nor COMMAND is
//! specified, the current security context is printed.」）
//!
//! 本專案不使用 libselinux：以 `/sys/fs/selinux` 是否存在判斷 SELinux kernel，
//! 不存在時照參考回 `runcon: runcon may be used only on a SELinux kernel`。

use crate::exec::{errno_reason, is_executable, resolve_program, status_code};
use crate::pathname;
use crate::{In, Out};

const RUNCON_HELP: &str = r#"Run command with specified security context under SELinux enabled systems.

Usage: runcon CONTEXT COMMAND [ARG...]
       runcon [-c] [-u USER] [-r ROLE] [-t TYPE] [-l RANGE] COMMAND [ARG...]

Arguments:
  [ARG]...  

Options:
  -c, --compute        Compute process transition context before modifying.
  -u, --user <USER>    Set user USER in the target security context.
  -r, --role <ROLE>    Set role ROLE in the target security context.
  -t, --type <TYPE>    Set type TYPE in the target security context.
  -l, --range <RANGE>  Set range RANGE in the target security context.
  -h, --help           Print help
  -V, --version        Print version

Run COMMAND with completely-specified CONTEXT, or with current or transitioned security context
modified by one or more of LEVEL, ROLE, TYPE, and USER.

If none of --compute, --type, --user, --role or --range is specified, then the first argument is
used as the complete context.

Note that only carefully-chosen contexts are likely to successfully run.

If neither CONTEXT nor COMMAND is specified, the current security context is printed.
"#;

const RUNCON_USAGE: &str =
    "runcon CONTEXT COMMAND [ARG...]\n       runcon [-c] [-u USER] [-r ROLE] [-t TYPE] [-l RANGE] COMMAND [ARG...]";
const RUNCON_LONGS: &[&str] = &["compute", "help", "range", "role", "type", "user", "version"];

const CONTEXT_PATH: &str = "/proc/self/attr/current";
const SELINUX_FS: &str = "/sys/fs/selinux";

fn unexpected(shown: &str, raw: &str, similar: Option<&str>, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found\n\n", shown).ok();
    if let Some(c) = similar {
        writeln!(stderr, "tip: a similar argument exists: '--{}'", c).ok();
    } else {
        writeln!(
            stderr,
            "  tip: to pass '{}' as a value, use '-- {}'",
            raw, raw
        )
        .ok();
    }
    write!(
        stderr,
        "\nUsage: {}\n\nFor more information, try '--help'.\n",
        RUNCON_USAGE
    )
    .ok();
    125
}

/// clap 不會把「看起來像旗標」的 token 當成選項值：已知旗標與裸 `--` → 「a value is
/// required」；未知短旗標 → 「unexpected argument」。`-`（裸）仍是合法值。
/// clap 不會把「看起來像旗標」的 token 當成選項值：已知旗標（含 `--`）→
/// 「a value is required」；未知旗標 → 「unexpected argument」。`-`（裸）仍是合法值。
fn value_for(
    long: &str,
    metavar: &str,
    attached: Option<String>,
    args: &[String],
    i: &mut usize,
    slot: &mut Option<String>,
    stderr: Out,
) -> Option<i32> {
    if let Some(v) = attached {
        *slot = Some(v);
        return None;
    }
    if *i >= args.len() {
        pathname::clap_value_required(long, metavar, stderr);
        return Some(125);
    }
    let next = args[*i].clone();
    if next.starts_with("--") {
        let name = next[2..].split('=').next().unwrap_or("").to_string();
        match pathname::resolve_long(&name, RUNCON_LONGS) {
            Ok(_) => {
                pathname::clap_value_required(long, metavar, stderr);
                return Some(125);
            }
            Err(None) => return Some(unexpected(&format!("--{}", name), &format!("--{}", name), None, stderr)),
            Err(Some(c)) => {
                return Some(unexpected(&format!("--{}", name), &format!("--{}", name), Some(&c), stderr))
            }
        }
    }
    if next.starts_with('-') && next.len() > 1 {
        // 未知短旗標（`-u -5`、`-l -s0`）：clap 照樣把它當成旗標而報 unknown。
        let is_known = matches!(next.as_bytes()[1], b'c' | b'h' | b'V' | b'u' | b'r' | b't' | b'l');
        if is_known {
            pathname::clap_value_required(long, metavar, stderr);
            return Some(125);
        }
        let shown = pathname::clap_unknown_short(&next, 1);
        let raw = if shown == "0" { "-0".to_string() } else { shown.clone() };
        return Some(unexpected(&shown, &raw, None, stderr));
    }
    *slot = Some(next);
    *i += 1;
    None
}



fn current_context() -> String {
    match std::fs::read_to_string(CONTEXT_PATH) {
        Ok(s) => s,
        Err(_) => String::new(),
    }
}

fn selinux_enabled() -> bool {
    std::path::Path::new(SELINUX_FS).exists()
}

/// 以現有 context 為基底替換指定_components_（`user:role:type:level`）。
fn compute_context(base: &str, user: &Option<String>, role: &Option<String>, typ: &Option<String>, range: &Option<String>) -> String {
    let body = base.trim_end_matches('\n');
    let mut parts: Vec<String> = body.split(':').map(|s| s.to_string()).collect();
    while parts.len() < 4 {
        parts.push(String::new());
    }
    if let Some(v) = user {
        parts[0] = v.clone();
    }
    if let Some(v) = role {
        parts[1] = v.clone();
    }
    if let Some(v) = typ {
        parts[2] = v.clone();
    }
    if let Some(v) = range {
        parts[3] = v.clone();
    }
    while parts.last().map(|p| p.is_empty()).unwrap_or(false) {
        parts.pop();
    }
    parts.join(":")
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut compute = false;
    let mut compute_given = false;
    let mut user: Option<String> = None;
    let mut role: Option<String> = None;
    let mut typ: Option<String> = None;
    let mut range: Option<String> = None;
    let mut operands: Vec<String> = Vec::new();
    let mut only_positional = false;
    let mut i = 0;

    while i < args.len() {
        let a = &args[i];
        i += 1;
        if only_positional {
            operands.push(a.clone());
            continue;
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let attached = a.split_once('=').map(|(_, v)| v.to_string());
            match pathname::resolve_long(&name, RUNCON_LONGS) {
                Ok("help") => {
                    if let Some(v) = &attached {
                        pathname::clap_unexpected_value("help", v, RUNCON_USAGE, stderr);
                        return 125;
                    }
                    write!(stdout, "{}", RUNCON_HELP).ok();
                    return 0;
                }
                Ok("version") => {
                    if let Some(v) = &attached {
                        pathname::clap_unexpected_value("version", v, RUNCON_USAGE, stderr);
                        return 125;
                    }
                    writeln!(stdout, "runcon (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                Ok("compute") => {
                    if let Some(v) = &attached {
                        pathname::clap_unexpected_value("compute", v, RUNCON_USAGE, stderr);
                        return 125;
                    }
                    if compute_given {
                        return pathname::clap_dup("compute", "", 125, stderr);
                    }
                    compute_given = true;
                    compute = true;
                }
                Ok(long @ ("user" | "role" | "type" | "range")) => {
                    let (metavar, slot): (&str, &mut Option<String>) = match long {
                        "user" => ("USER", &mut user),
                        "role" => ("ROLE", &mut role),
                        "type" => ("TYPE", &mut typ),
                        _ => ("RANGE", &mut range),
                    };
                    if slot.is_some() {
                        return pathname::clap_dup(long, metavar, 125, stderr);
                    }
                    if let Some(code) = value_for(long, metavar, attached, args, &mut i, slot, stderr) {
                        return code;
                    }
                }
                Ok(other) => {
                    let shown = format!("--{}", other);
                    return unexpected(&shown, &shown, None, stderr);
                }
                Err(similar) => return unexpected(a, a, similar, stderr),
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let mut k = 1;
            while k < a.len() {
                match a.as_bytes()[k] {
                    b'c' => {
                        if compute_given {
                            return pathname::clap_dup("compute", "", 125, stderr);
                        }
                        compute_given = true;
                        compute = true;
                    }
                    b'h' => {
                        write!(stdout, "{}", RUNCON_HELP).ok();
                        return 0;
                    }
                    b'V' => {
                        writeln!(stdout, "runcon (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    short @ (b'u' | b'r' | b't' | b'l') => {
                        let (long, metavar, slot): (&str, &str, &mut Option<String>) = match short {
                            b'u' => ("user", "USER", &mut user),
                            b'r' => ("role", "ROLE", &mut role),
                            b't' => ("type", "TYPE", &mut typ),
                            _ => ("range", "RANGE", &mut range),
                        };
                        if slot.is_some() {
                            return pathname::clap_dup(long, metavar, 125, stderr);
                        }
                        let attached = if k + 1 < a.len() {
                            Some(a[k + 1..].to_string())
                        } else {
                            None
                        };
                        if let Some(v) = attached {
                            *slot = Some(v);
                            k = a.len();
                            continue;
                        }
                        if let Some(code) = value_for(long, metavar, None, args, &mut i, slot, stderr) {
                            return code;
                        }
                    }
                    b'=' => {
                        // `-c=1`：clap 把 `=` 當成未知旗標字元（顯示 `-=``）。
                        return unexpected(&a[k..=k], &a[k..=k], None, stderr);
                    }
                    _ => {
                        // clap 在 tip 裡用的是「顯示用的那個 token」（`-cfoo` → tip 用 `-f`；
                        // `-c0` 顯示 '0' 但 tip 用 `-0`）。
                        let shown = pathname::clap_unknown_short(a, k);
                        let raw = if shown == "0" { "-0".to_string() } else { shown.clone() };
                        return unexpected(&shown, &raw, None, stderr);
                    }
                }
                k += 1;
            }
            continue;
        }
        operands.push(a.clone());
    }

    if operands.is_empty() {
        // man：「If neither CONTEXT nor COMMAND is specified, the current security context is printed.」
        writeln!(stdout, "{}", current_context().trim_end_matches('\n')).ok();
        return 0;
    }

    let has_flags = compute || user.is_some() || role.is_some() || typ.is_some() || range.is_some();
    let (context, command, cmd_args) = if has_flags {
        // 有旗標時：operand[0] 是 COMMAND，context 由現有 context 套用給定的 components。
        let context = compute_context(&current_context(), &user, &role, &typ, &range);
        (context, operands[0].clone(), operands[1..].to_vec())
    } else {
        let context = operands[0].clone();
        if operands.len() < 2 {
            writeln!(stderr, "runcon: No command is specified").ok();
            return 1;
        }
        (context, operands[1].clone(), operands[2..].to_vec())
    };

    if !selinux_enabled() {
        writeln!(stderr, "runcon: runcon may be used only on a SELinux kernel").ok();
        return 1;
    }
    // SELinux kernel：把 context 寫入 /proc/self/attr/current（setcon 的 procfs 入口）。
    if let Err(e) = std::fs::write(CONTEXT_PATH, context.as_str()) {
        writeln!(
            stderr,
            "runcon: setting context to {}: {}",
            context,
            crate::exec::errno_reason(&e)
        )
        .ok();
        return 1;
    }
    match resolve_program(&command) {
        Some(prog) if is_executable(&prog) => {
            let mut builder = std::process::Command::new(&prog);
            builder.args(&cmd_args);
            match builder.status() {
                Ok(status) => status_code(status),
                Err(e) => {
                    writeln!(stderr, "runcon: {}: {}", command, errno_reason(&e)).ok();
                    126
                }
            }
        }
        Some(_) => {
            writeln!(stderr, "runcon: {}: Permission denied", command).ok();
            126
        }
        None => {
            writeln!(
                stderr,
                "runcon: {}: No such file or directory",
                command
            )
            .ok();
            127
        }
    }
}
