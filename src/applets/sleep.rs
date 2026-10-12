//! sleep(1) — Pause for NUMBER seconds.
//!
//! 依據 man page（docs/man/sleep.1.gz）：
//! - `SUFFIX may be 's' for seconds (the default), 'm' for minutes, 'h' for hours or 'd' for days`
//! - `NUMBER may be an arbitrary floating point number`
//! - `Given two or more arguments, pause for the amount of time specified by the sum of their values`
//!
//! 參考實測（本機 uutils coreutils 0.10.0）：
//! - 未知選項 → clap 措辭（tip/Usage/For more），rc=1
//! - 無法解析 → `sleep: invalid time interval 'X'` + Try 行，rc=1
//! - 無 operand → `sleep: missing operand` + Try 行，rc=1
//! - 選項解析貫穿 positional（`sleep 1 --help` → help）
//! 偏差：參考把 `0x10` 當成 C strtod 的 16；本實作一律拒绝十六進位（記入 assumptions）。

use crate::pathname::usage_hint;
use crate::{In, Out};

const SLEEP_HELP: &str = r#"Pause for NUMBER seconds.

Usage: sleep NUMBER[SUFFIX]...
       sleep OPTION

Arguments:
  [NUMBER]...  pause for NUMBER seconds

Options:
  -h, --help     Print help
  -V, --version  Print version

Pause for NUMBER seconds. SUFFIX may be 's' for seconds (the default),
'm' for minutes, 'h' for hours or 'd' for days. Unlike most implementations
that require NUMBER be an integer, here NUMBER may be an arbitrary floating
point number. Given two or more arguments, pause for the amount of time
specified by the sum of their values."#;

const SLEEP_USAGE: &str = "sleep NUMBER[SUFFIX]...\n       sleep OPTION";

fn clap_unexpected(arg: &str, stderr: Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "  tip: to pass '{}' as a value, use '-- {}'", arg, arg).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "Usage: {}", SLEEP_USAGE).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}

/// 解析一個 NUMBER[SUFFIX]；回 None 表示措辭上的「invalid time interval」。
fn parse_interval(s: &str) -> Option<f64> {
    let mut body = s;
    let mut mult = 1.0f64;
    if let Some(c) = s.chars().next_back() {
        let m = match c {
            's' => Some(1.0f64),
            'm' => Some(60.0),
            'h' => Some(3600.0),
            'd' => Some(86400.0),
            _ => None,
        };
        if let Some(m) = m {
            mult = m;
            body = &s[..s.len() - c.len_utf8()];
        }
    }
    let v: f64 = body.parse().ok()?;
    if !v.is_finite() || v < 0.0 {
        return None;
    }
    Some(v * mult)
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut operands: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        i += 1;
        if no_more_opts || (!a.starts_with('-') || a == "-") {
            operands.push(a);
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            match crate::pathname::infer_long_opt(&name, &["help", "version"]) {
                Some("help") => {
                    writeln!(stdout, "{}", SLEEP_HELP).ok();
                    return 0;
                }
                Some("version") => {
                    writeln!(stdout, "sleep (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => return clap_unexpected(&a, stderr),
            }
        }
        for c in a[1..].chars() {
            match c {
                'h' => {
                    writeln!(stdout, "{}", SLEEP_HELP).ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "sleep (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => return clap_unexpected(&format!("-{}", c), stderr),
            }
        }
    }

    if operands.is_empty() {
        writeln!(stderr, "sleep: missing operand").ok();
        writeln!(stderr, "{}", usage_hint("sleep")).ok();
        return 1;
    }
    let mut total = 0.0f64;
    for op in &operands {
        match parse_interval(op) {
            Some(v) => total += v,
            None => {
                writeln!(stderr, "sleep: invalid time interval '{}'", op).ok();
                writeln!(stderr, "{}", usage_hint("sleep")).ok();
                return 1;
            }
        }
    }
    // 分塊睡：避免 Duration::from_secs_f64 對超大值 panic（總和可為 inf 級別的浮點）。
    let mut remaining = total;
    while remaining > 0.0 {
        let step = if remaining > 3600.0 { 3600.0 } else { remaining };
        std::thread::sleep(std::time::Duration::from_secs_f64(step));
        remaining -= step;
    }
    0
}
