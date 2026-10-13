//! pinky — 印出使用者的簡短資訊（finger-lite）。
//!
//! 規格來源：`docs/man/pinky.1.gz`。man 只定義選項語意，未定義欄位寬度、
//! 時間格式、未知使用者的訊息措辭 → 這些是我方決定（見 docs/assumptions.md「pinky」節）：
//! - 短格式欄位：USER、TTY、IDLE、TIME、LOGIN FROM（固定寬 9/9/6/17，行尾不去空白）。
//! - USER 欄預設顯示 passwd gecos 的第一欄（full name）；無 gecos 時退回登入名。
//!   -w 改顯示登入名；-i 省略 USER 與 LOGIN FROM；-q 再省略 IDLE。
//! - 時間格式沿用 who 的 `%Y-%m-%d %H:%M`；IDLE 沿用 who：>1 天為 `old`，否則 HH:MM，<60 秒為 `.`。
//! - 未知使用者：`pinky: unknown user '<operand>'`（stderr），繼續處理其他 operand，最終 rc 1。
//! - `-h` 是「省略 project 檔」，不是 help（man 的 help 只有長形 `--help`）。

use crate::passwd;
use crate::pathname;
use crate::sys;
use crate::utmp;
use crate::{In, Out};

const PINKY_HELP: &str = r#"Displays brief user information on Unix-based systems

Usage: pinky [OPTION]... [USER]...

Options:
  -l               produce long format output for the specified USERs
  -b               omit the user's home directory and shell in long format
  -h               omit the user's project file in long format
  -p               omit the user's plan file in long format
  -s               do short format output, this is the default
  -f               omit the line of column headings in short format
  -w               omit the user's full name in short format
  -i               omit the user's full name and remote host in short format
  -q               omit the user's full name, remote host and idle time in short format
      --lookup     attempt to canonicalize hostnames via DNS
      --help       Print help
  -V, --version    Print version

Arguments:
  [user]
"#;

const PINKY_LONGS: &[&str] = &["help", "lookup", "version"];

fn unexpected(arg: &str, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found\n\n", arg).ok();
    write!(
        stderr,
        "Usage: pinky [OPTION]... [USER]...\n\nFor more information, try '--help'.\n"
    )
    .ok();
    2
}

fn dup(flag: &str, stderr: Out) -> i32 {
    write!(
        stderr,
        "error: the argument '{}' cannot be used multiple times\n\nFor more information, try '--help'.\n",
        flag
    )
    .ok();
    2
}

#[derive(Default)]
struct Cfg {
    long: bool,
    no_home_shell: bool,
    no_project: bool,
    no_plan: bool,
    no_heading: bool,
    no_full_name: bool,
    no_name_host: bool,
    no_idle: bool,
    #[allow(dead_code)]
    lookup: bool,
}

fn gecos_name(gecos: &str, fallback: &str) -> String {
    let first = gecos.split(',').next().unwrap_or("");
    if first.is_empty() {
        fallback.to_string()
    } else {
        first.to_string()
    }
}

fn fmt_time(secs: i32) -> String {
    match sys::local_time(secs as i64) {
        Some((y, mo, d, h, mi)) => format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, d, h, mi),
        None => String::new(),
    }
}

fn idle_text(secs: i32, now: i64) -> String {
    let delta = now - secs as i64;
    if delta < 60 {
        ".".to_string()
    } else if delta > 86400 {
        "old".to_string()
    } else {
        format!("{:02}:{:02}", delta / 3600, (delta % 3600) / 60)
    }
}

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - n))
    }
}

fn rtrim(s: &mut String) {
    while s.ends_with(' ') {
        s.pop();
    }
}

fn user_processes() -> Vec<utmp::Record> {
    utmp::read_records(utmp::DEFAULT_PATH)
        .into_iter()
        .filter(|r| r.utype == utmp::USER_PROCESS)
        .collect()
}

fn short_output(cfg: &Cfg, users: &[String], stdout: Out, stderr: Out) -> i32 {
    // man 的 [user] operand：我方決定短格式依 operand 過濾 utmp（見 assumptions）。
    let recs: Vec<utmp::Record> = if users.is_empty() {
        user_processes()
    } else {
        user_processes().into_iter().filter(|r| users.iter().any(|u| u == &r.user)).collect()
    };
    let mut rc = 0;
    for u in users {
        if !recs.iter().any(|r| &r.user == u) && passwd::passwd_by_operand(u).is_none() {
            write!(stderr, "pinky: unknown user '{}'\n", u).ok();
            rc = 1;
        }
    }
    let now = sys::now_epoch();
    if !cfg.no_heading {
        let mut cols: Vec<String> = Vec::new();
        if !cfg.no_name_host {
            cols.push(pad("USER", 9));
        }
        cols.push(pad("TTY", 9));
        if !cfg.no_idle {
            cols.push(pad("IDLE", 6));
        }
        cols.push(pad("TIME", 17));
        if !cfg.no_name_host {
            cols.push("LOGIN FROM".to_string());
        }
        let mut line = cols.join("");
        rtrim(&mut line);
        writeln!(stdout, "{}", line).ok();
    }
    for r in &recs {
        let mut cols: Vec<String> = Vec::new();
        if !cfg.no_name_host {
        let display = if cfg.no_full_name {
                r.user.clone()
            } else {
                match passwd::passwd_by_name(&r.user) {
                    Some(p) => gecos_name(&p.gecos, &r.user),
                    None => r.user.clone(),
                }
            };
            // USER 欄最寬 9：名稱截到 8 字元＋一格空格以維持欄位對齊（我方決定）。
            let display: String = display.chars().take(8).collect();
            cols.push(pad(&display, 9));
        }
        cols.push(pad(&r.line, 9));
        if !cfg.no_idle {
            cols.push(pad(&idle_text(r.tv_sec, now), 6));
        }
        cols.push(pad(&fmt_time(r.tv_sec), 17));
        if !cfg.no_name_host {
            cols.push(r.host.clone());
        }
        let mut line = cols.join("");
        rtrim(&mut line);
        writeln!(stdout, "{}", line).ok();
    }
    rc
}

fn content_block(path: &str, label: &str, stdout: Out) {
    let text = match std::fs::read_to_string(path) {
        Ok(t) => t,
        Err(_) => return,
    };
    if text.trim().is_empty() {
        return;
    }
    writeln!(stdout, "{}", label).ok();
    write!(stdout, "{}", text).ok();
    if !text.ends_with('\n') {
        writeln!(stdout).ok();
    }
}

fn long_block(name_line: &str, real_name: &str, dir: &str, shell: &str, has_passwd: bool, user: &str, cfg: &Cfg, stdout: Out) {
    writeln!(stdout, "Login name: {}", name_line).ok();
    writeln!(stdout, "In real life: {}", real_name).ok();
    if has_passwd && !cfg.no_home_shell {
        writeln!(stdout, "Directory: {}", dir).ok();
        writeln!(stdout, "Shell: {}", shell).ok();
    }
    for r in user_processes().into_iter().filter(|r| r.user == user) {
        let host = if r.host.is_empty() {
            String::new()
        } else {
            format!(" ({})", r.host)
        };
        writeln!(stdout, "On since {} (UTC) on {}{}", fmt_time(r.tv_sec), r.line, host).ok();
    }
    if has_passwd && !cfg.no_project {
        content_block(&format!("{}/.project", dir), "Project:", stdout);
    }
    if has_passwd && !cfg.no_plan {
        content_block(&format!("{}/.plan", dir), "Plan:", stdout);
    }
}

fn long_output(users: &[String], cfg: &Cfg, stdout: Out, stderr: Out) -> i32 {
    let mut rc = 0;
    let mut blocks: Vec<String> = Vec::new();
    if users.is_empty() {
        for r in user_processes() {
            let mut buf = Vec::new();
            match passwd::passwd_by_name(&r.user) {
                Some(p) => long_block(&p.name, &gecos_name(&p.gecos, &p.name), &p.dir, &p.shell, true, &r.user, cfg, &mut buf),
                None => long_block(&r.user, &r.user, "", "", false, &r.user, cfg, &mut buf),
            }
            blocks.push(String::from_utf8_lossy(&buf).into_owned());
        }
    } else {
        for operand in users {
            match passwd::passwd_by_operand(operand) {
                Some(p) => {
                    let mut buf = Vec::new();
                    long_block(&p.name, &gecos_name(&p.gecos, &p.name), &p.dir, &p.shell, true, &p.name, cfg, &mut buf);
                    blocks.push(String::from_utf8_lossy(&buf).into_owned());
                }
                None => {
                    let mut buf = Vec::new();
                    long_block(operand, operand, "", "", false, operand, cfg, &mut buf);
                    blocks.push(String::from_utf8_lossy(&buf).into_owned());
                    write!(stderr, "pinky: unknown user '{}'\n", operand).ok();
                    rc = 1;
                }
            }
        }
    }
    for (i, b) in blocks.iter().enumerate() {
        if i > 0 {
            writeln!(stdout).ok();
        }
        write!(stdout, "{}", b).ok();
    }
    rc
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut cfg = Cfg::default();
    let mut seen: Vec<&str> = Vec::new();
    let mut users: Vec<String> = Vec::new();
    let mut only_positional = false;
    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        i += 1;
        if only_positional {
            users.push(a.clone());
            continue;
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if a == "--help" {
            write!(stdout, "{}", PINKY_HELP).ok();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stdout, "pinky (uutils coreutils) 0.10.0").ok();
            return 0;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let value = a.split_once('=').map(|(_, v)| v.to_string());
            match pathname::resolve_long(&name, PINKY_LONGS) {
                Ok("help") => {
                    if let Some(v) = value {
                        return pathname::clap_unexpected_value("help", &v, "pinky", stderr);
                    }
                    write!(stdout, "{}", PINKY_HELP).ok();
                    return 0;
                }
                Ok("version") => {
                    if let Some(v) = value {
                        return pathname::clap_unexpected_value("version", &v, "pinky", stderr);
                    }
                    writeln!(stdout, "pinky (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                Ok("lookup") => {
                    if let Some(v) = value {
                        return pathname::clap_unexpected_value("lookup", &v, "pinky", stderr);
                    }
                    if seen.contains(&"lookup") {
                        return dup("--lookup", stderr);
                    }
                    seen.push("lookup");
                    cfg.lookup = true;
                }
                Ok(_) => return unexpected(a, stderr),
                Err(Some(c)) => {
                    write!(stderr, "error: unexpected argument '{}' found\n\n  tip: a similar argument exists: '--{}'\n\nUsage: pinky [OPTION]... [USER]...\n\nFor more information, try '--help'.\n", a, c).ok();
                    return 2;
                }
                Err(None) => return unexpected(a, stderr),
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 && a != "-" {
            let chars: Vec<char> = a[1..].chars().collect();
            let mut k = 0;
            while k < chars.len() {
                let flag = match chars[k] {
                    'l' => "l",
                    'b' => "b",
                    'h' => "h",
                    'p' => "p",
                    's' => "s",
                    'f' => "f",
                    'w' => "w",
                    'i' => "i",
                    'q' => "q",
                    'V' => {
                        writeln!(stdout, "pinky (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    _ => {
                        return unexpected(&format!("-{}", chars[k]), stderr);
                    }
                };
                if seen.contains(&flag) {
                    return dup(&format!("-{}", flag), stderr);
                }
                seen.push(flag);
                match flag {
                    "l" => cfg.long = true,
                    "b" => cfg.no_home_shell = true,
                    "h" => cfg.no_project = true,
                    "p" => cfg.no_plan = true,
                    "s" => cfg.long = false,
                    "f" => cfg.no_heading = true,
                    "w" => cfg.no_full_name = true,
                    "i" => cfg.no_name_host = true,
                    "q" => {
                        cfg.no_name_host = true;
                        cfg.no_idle = true;
                    }
                    _ => {}
                }
                k += 1;
            }
            continue;
        }
        users.push(a.clone());
    }

    if cfg.long {
        return long_output(&users, &cfg, stdout, stderr);
    }
    short_output(&cfg, &users, stdout, stderr)
}

