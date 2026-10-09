//! echo(1) — Display a line of text.
//!
//! 依據 man page（docs/man/echo.1.gz）：
//! - `STRING(s)` 用空白分隔輸出到 stdout，結尾加 newline
//! - `-n` 不輸出結尾 newline
//! - `-e` 啟用 backslash escape；`-E` 關閉（預設）
//! - `-h`/`--help`、`-V`/`--version`
//! - `-e` 時辨識：\\ \a \b \c \e \f \n \r \t \v \0NNN \xHH
//! - `\c` 表示「不再輸出任何東西」（含結尾 newline）
//!
//! man page 沒定義未知選項的行為；這裡把不認識的 `-xxx` 當成一般 STRING（記錄在 docs/assumptions.md）。

use crate::{In, Out};

pub fn run(args: &[String], _stdin: In, stdout: Out, _stderr: Out) -> i32 {
    let mut no_newline = false;
    let mut escapes = false;
    let mut strings: Vec<String> = Vec::new();

    for a in args {
        if a == "-h" || a == "--help" {
            writeln!(stdout, "Usage: echo [-n] [-e] [-E] [-h|--help] [-V|--version] [STRING]").ok();
            return 0;
        }
        if a == "-V" || a == "--version" {
            writeln!(stdout, "echo (rust-coreutils) 0.1.0").ok();
            return 0;
        }
        if a.starts_with('-') && a.len() > 1 && a != "--" {
            // man page 只列 -n -e -E -h -V；若字串含未知字元，視為 STRING（docs/assumptions.md）
            let known = a[1..].chars().all(|c| matches!(c, 'n' | 'e' | 'E'));
            if !known {
                strings.push(a.clone());
                continue;
            }
            for ch in a[1..].chars() {
                match ch {
                    'n' => no_newline = true,
                    'e' => escapes = true,
                    'E' => escapes = false,
                    _ => {}
                }
            }
            continue;
        }
        strings.push(a.clone());
    }

    for (i, s) in strings.iter().enumerate() {
        if i > 0 {
            write!(stdout, " ").ok();
        }
        if escapes {
            let (out, stop) = expand(s);
            write!(stdout, "{out}").ok();
            if stop {
                return 0;
            }
        } else {
            write!(stdout, "{s}").ok();
        }
    }

    if !no_newline {
        writeln!(stdout).ok();
    }
    0
}

/// 回傳 (展開結果, 是否因 \c 停止)。
fn expand(s: &str) -> (String, bool) {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        let Some(n) = it.next() else {
            out.push('\\');
            continue;
        };
        match n {
            '\\' => out.push('\\'),
            'a' => out.push('\x07'),
            'b' => out.push('\x08'),
            'c' => return (out, true),
            'e' => out.push('\x1B'),
            'f' => out.push('\x0C'),
            'n' => out.push('\n'),
            'r' => out.push('\r'),
            't' => out.push('\t'),
            'v' => out.push('\x0B'),
            '0'..='9' => {
                let mut digits = String::new();
                digits.push(n);
                while digits.len() < 3 {
                    match it.peek() {
                        Some(d) if d.is_ascii_digit() => {
                            digits.push(it.next().unwrap());
                        }
                        _ => break,
                    }
                }
                let v = u32::from_str_radix(&digits, 8).unwrap_or(0);
                out.push((v as u8) as char);
            }
            'x' => {
                let mut digits = String::new();
                while digits.len() < 2 {
                    match it.peek() {
                        Some(d) if d.is_ascii_hexdigit() => {
                            digits.push(it.next().unwrap());
                        }
                        _ => break,
                    }
                }
                if digits.is_empty() {
                    out.push('x');
                } else {
                    let v = u32::from_str_radix(&digits, 16).unwrap_or(0);
                    out.push((v as u8) as char);
                }
            }
            other => {
                out.push('\\');
                out.push(other);
            }
        }
    }
    (out, false)
}
