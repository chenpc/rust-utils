//! wc(1) — Print newline, word, and byte counts for each FILE.
//!
//! 依據 man page（docs/man/wc.1.gz）：
//! - 預設印 lines / words / bytes
//! - `-c|--bytes`、`-m|--chars`、`-l|--lines`、`-L|--max-line-length`、`-w|--words`
//! - 超過一個檔案時印 total 行；`--total <auto|always|only|never>`
//! - `--files0-from F`：讀 NUL 分隔的檔名（F 為 `-` 時從 stdin 讀）
//! - `-h|--help`、`-V|--version`
//!
//! man page 未定義：word 的定義、數字是否補零對齊、`-L` 在輸出中的位置。
//! 假設（docs/assumptions.md）：word = 非空白字元的連續段；數字用單空格分隔不補零；
//! 輸出順序 = lines, words, chars/bytes, max-line-length。

use std::io::Read;

use crate::{In, Out};

#[derive(Default, Clone, Copy)]
struct Counts {
    lines: u64,
    words: u64,
    bytes: u64,
    chars: u64,
    maxlen: u64,
}

fn count(buf: &[u8]) -> Counts {
    let mut c = Counts::default();
    c.bytes = buf.len() as u64;
    c.chars = String::from_utf8_lossy(buf).chars().count() as u64;
    let mut run = 0u64;
    let mut in_word = false;
    for &b in buf {
        if b == b'\n' {
            c.lines += 1;
            if run > c.maxlen {
                c.maxlen = run;
            }
            run = 0;
            in_word = false;
            continue;
        }
        run += 1;
        if b.is_ascii_whitespace() {
            in_word = false;
        } else if !in_word {
            c.words += 1;
            in_word = true;
        }
    }
    if run > c.maxlen {
        c.maxlen = run;
    }
    c
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut show_lines = false;
    let mut show_words = false;
    let mut show_bytes = false;
    let mut show_chars = false;
    let mut show_max = false;
    let mut any_flag = false;
    let mut total_mode = String::from("auto");
    let mut files0_from: Option<String> = None;
    let mut files: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        match a.as_str() {
            "-h" | "--help" => {
                writeln!(stdout, "Usage: wc [-c] [-m] [--files0-from F] [-l] [-L] [--total WHEN] [-w] [-h|--help] [-V|--version] [files]").ok();
                return 0;
            }
            "-V" | "--version" => {
                writeln!(stdout, "wc (rust-coreutils) 0.1.0").ok();
                return 0;
            }
            "--total" => {
                if i + 1 < args.len() {
                    total_mode = args[i + 1].clone();
                    i += 1;
                }
            }
            "--files0-from" => {
                if i + 1 < args.len() {
                    files0_from = Some(args[i + 1].clone());
                    i += 1;
                }
            }
            s if s.starts_with('-') && s != "-" && s.len() > 1 => {
                any_flag = true;
                for ch in s[1..].chars() {
                    match ch {
                        'c' => show_bytes = true,
                        'm' => show_chars = true,
                        'l' => show_lines = true,
                        'L' => show_max = true,
                        'w' => show_words = true,
                        _ => {}
                    }
                }
            }
            other => files.push(other.to_string()),
        }
        i += 1;
    }

    if !any_flag {
        show_lines = true;
        show_words = true;
        show_bytes = true;
    }

    if let Some(f) = files0_from.clone() {
        if f == "-" {
            let mut buf = Vec::new();
            stdin.read_to_end(&mut buf).ok();
            for name in String::from_utf8_lossy(&buf).split('\0') {
                if !name.is_empty() {
                    files.push(name.to_string());
                }
            }
        } else {
            let mut buf = Vec::new();
            match std::fs::File::open(&f) {
                Ok(mut fh) => {
                    fh.read_to_end(&mut buf).ok();
                }
                Err(e) => {
                    writeln!(stderr, "wc: {f}: {e}").ok();
                    return 1;
                }
            }
            for name in String::from_utf8_lossy(&buf).split('\0') {
                if !name.is_empty() {
                    files.push(name.to_string());
                }
            }
        }
    }

    let mut total = Counts::default();
    let mut status = 0;
    for name in files.iter() {
        let mut buf = Vec::new();
        if name == "-" {
            if stdin.read_to_end(&mut buf).is_err() {
                writeln!(stderr, "wc: {name}: read error").ok();
                status = 1;
                continue;
            }
        } else {
            match std::fs::File::open(name) {
                Ok(mut fh) => {
                    if fh.read_to_end(&mut buf).is_err() {
                        writeln!(stderr, "wc: {name}: read error").ok();
                        status = 1;
                        continue;
                    }
                }
                Err(e) => {
                    writeln!(stderr, "wc: {name}: {e}").ok();
                    status = 1;
                    continue;
                }
            }
        }
        let c = count(&buf);
        total.lines += c.lines;
        total.words += c.words;
        total.bytes += c.bytes;
        total.chars += c.chars;
        total.maxlen = total.maxlen.max(c.maxlen);
        if total_mode != "only" {
            print_counts(stdout, &c, show_lines, show_words, show_bytes, show_chars, show_max, Some(name));
        }
    }
    if total_mode != "never" && (total_mode == "always" || total_mode == "only" || files.len() > 1) {
        print_counts(stdout, &total, show_lines, show_words, show_bytes, show_chars, show_max, Some("total"));
    }

    if files.is_empty() {
        let mut buf = Vec::new();
        if stdin.read_to_end(&mut buf).is_err() {
            writeln!(stderr, "wc: read error").ok();
            return 1;
        }
        let c = count(&buf);
        print_counts(stdout, &c, show_lines, show_words, show_bytes, show_chars, show_max, None);
        if total_mode == "always" {
            print_counts(stdout, &c, show_lines, show_words, show_bytes, show_chars, show_max, Some("total"));
        }
    }

    status
}

fn print_counts(
    stdout: Out,
    c: &Counts,
    lines: bool,
    words: bool,
    bytes: bool,
    chars: bool,
    max: bool,
    label: Option<&str>,
) {
    let mut parts: Vec<String> = Vec::new();
    if lines {
        parts.push(c.lines.to_string());
    }
    if words {
        parts.push(c.words.to_string());
    }
    if bytes {
        parts.push(c.bytes.to_string());
    }
    if chars {
        parts.push(c.chars.to_string());
    }
    if max {
        parts.push(c.maxlen.to_string());
    }
    let line = match label {
        Some(l) => format!("{} {l}", parts.join(" ")),
        None => parts.join(" "),
    };
    writeln!(stdout, "{line}").ok();
}
