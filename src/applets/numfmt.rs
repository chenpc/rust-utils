//! numfmt：依 man page（docs/man/numfmt.1.gz）實作。
//!
//! 單位系統：none / auto / si / iec / iec-i。
//!   si:     1K = 1000, 1M = 1000000, ...
//!   iec:    1K = 1024, 1M = 1048576, ...
//!   iec-i:  1Ki = 1024, 1Mi = 1048576, ...
//!   auto:   單字或雙字後綴皆可（1K = 1000, 1Ki = 1024）。
//!
//! 假設（記錄於 docs/assumptions.md）：
//! - 預設 precision：有縮放時為 1，無縮放時沿用輸入的小數位數。
//! - `--format` 支援 `%f`（含 `%'f`、寬度、`%.Nf`、`%-Nf`）與整數 `%d/%i/%u`。
//! - `--invalid` 預設 abort：診斷寫 stderr 並立即以 2 結束。

use crate::In;
use crate::Out;

#[derive(Clone, Copy, PartialEq)]
enum Unit {
    None,
    Auto,
    Si,
    Iec,
    IecI,
}

#[derive(Clone, Copy, PartialEq)]
enum Round {
    Up,
    Down,
    FromZero,
    TowardsZero,
    Nearest,
}

const SI_LABELS: &[&str] = &["", "K", "M", "G", "T", "P", "E", "Z", "Y"];

fn unit_of(s: &str) -> Option<Unit> {
    match s {
        "none" => Some(Unit::None),
        "auto" => Some(Unit::Auto),
        "si" => Some(Unit::Si),
        "iec" => Some(Unit::Iec),
        "iec-i" => Some(Unit::IecI),
        _ => None,
    }
}

fn round_of(s: &str) -> Option<Round> {
    match s {
        "up" => Some(Round::Up),
        "down" => Some(Round::Down),
        "from-zero" => Some(Round::FromZero),
        "towards-zero" => Some(Round::TowardsZero),
        "nearest" => Some(Round::Nearest),
        _ => None,
    }
}

fn round_val(v: f64, method: Round) -> f64 {
    match method {
        Round::Up => v.ceil(),
        Round::Down => v.floor(),
        Round::FromZero => {
            if v < 0.0 {
                v.floor()
            } else {
                v.ceil()
            }
        }
        Round::TowardsZero => {
            if v < 0.0 {
                v.ceil()
            } else {
                v.floor()
            }
        }
        Round::Nearest => {
            let f = v.floor();
            let diff = v - f;
            if diff > 0.5 {
                f + 1.0
            } else if diff < 0.5 {
                f
            } else if (f as i64) % 2 == 0 {
                f
            } else {
                f + 1.0
            }
        }
    }
}

fn label(unit: Unit, exp: usize) -> String {
    if exp == 0 || exp >= SI_LABELS.len() {
        return String::new();
    }
    let base = SI_LABELS[exp].to_string();
    if unit == Unit::IecI {
        base + "i"
    } else {
        base
    }
}

fn suffix_value(s: &str, unit: Unit, unit_size: f64) -> Option<f64> {
    if s.is_empty() {
        return Some(1.0);
    }
    match unit {
        Unit::None => None,
        Unit::Si | Unit::Iec => {
            let exp = SI_LABELS.iter().position(|&l| l == s)?;
            let base: f64 = if unit == Unit::Si { 1000.0 } else { 1024.0 };
            Some(unit_size * base.powi(exp as i32))
        }
        Unit::IecI => {
            if s.len() < 2 || !s.ends_with('i') {
                return None;
            }
            let exp = SI_LABELS.iter().position(|&l| l == &s[..1])?;
            Some(unit_size * (1024.0f64).powi(exp as i32 - 1))
        }
        Unit::Auto => {
            if s.len() == 2 {
                if s.ends_with('i') {
                    let exp = SI_LABELS.iter().position(|&l| l == &s[..1])?;
                    return Some(unit_size * (1024.0f64).powi(exp as i32));
                }
                return None;
            }
            let exp = SI_LABELS.iter().position(|&l| l == s)?;
            Some(unit_size * (1000.0f64).powi(exp as i32))
        }
    }
}

fn numeric_body(s: &str) -> String {
    let mut keep = String::new();
    for c in s.chars() {
        if c.is_ascii_digit() || c == '.' || c == '-' || c == '+' {
            keep.push(c);
        } else {
            break;
        }
    }
    keep
}

fn parse_number(s: &str, unit: Unit, unit_size: f64, suffix: &str) -> Option<f64> {
    let mut body = s.to_string();
    let mut mult = 1.0;
    if !suffix.is_empty() && body.ends_with(suffix) {
        body = body[..body.len() - suffix.len()].to_string();
    }
    if unit != Unit::None {
        let mut found = false;
        for len in [2usize, 1usize] {
            if body.len() > len {
                let tail = body[body.len() - len..].to_string();
                if let Some(m) = suffix_value(&tail, unit, unit_size) {
                    body = body[..body.len() - len].to_string();
                    mult = m;
                    found = true;
                    break;
                }
            }
        }
        if !found && unit == Unit::None {
            return None;
        }
    }
    let v: f64 = body.parse().ok()?;
    Some(v * mult)
}

fn format_number(v: f64, precision: usize, grouping: bool, round: Round) -> String {
    let neg = v < 0.0;
    let a = v.abs();
    let pow = (10f64).powi(precision as i32);
    let scaled = round_val(a * pow, round);
    let ipart = (scaled / pow).trunc();
    let frac = (scaled - ipart * pow).round().abs() as i64;
    let mut text = if precision == 0 {
        format!("{}", ipart as i64)
    } else {
        format!("{}.{:0width$}", ipart as i64, frac, width = precision)
    };
    if grouping {
        let digits_start = if text.starts_with('-') { 1 } else { 0 };
        let int_part = text[..digits_start].to_string();
        let rest = text[digits_start..].replace(",", "");
        let whole = rest.split('.').next().unwrap_or("").to_string();
        let mut grouped = String::new();
        let bytes: Vec<char> = whole.chars().collect();
        for (i, c) in bytes.iter().enumerate() {
            if i > 0 && (bytes.len() - i) % 3 == 0 {
                grouped.push(',');
            }
            grouped.push(*c);
        }
        let tail = if rest.contains('.') {
            format!(".{}", rest.split('.').nth(1).unwrap_or(""))
        } else {
            String::new()
        };
        text = format!("{}{}{}", int_part, grouped, tail);
    }
    if neg {
        text.insert(0, '-');
    }
    text
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut from = Unit::None;
    let mut to = Unit::None;
    let mut from_size = 1.0f64;
    let mut to_size = 1.0f64;
    let mut format = String::new();
    let mut fields: Vec<(usize, Option<usize>)> = vec![(1, Some(1))];
    let mut padding: i64 = 0;
    let mut delimiter = ' ';
    let mut grouping = false;
    let mut suffix = String::new();
    let mut unit_sep = String::new();
    let mut header: usize = 0;
    let mut round = Round::FromZero;
    let mut zero = false;
    let mut numbers: Vec<String> = Vec::new();

    // --opt=value 先拆成兩個 token，後續解析只看 token 序列
    let mut norm: Vec<String> = Vec::new();
    for a in args {
        if let Some((name, val)) = a.split_once('=') {
            if name.starts_with("--") {
                norm.push(name.to_string());
                norm.push(val.to_string());
                continue;
            }
        }
        norm.push(a.to_string());
    }

    let mut i = 0;
    while i < norm.len() {
        let a = norm[i].as_str();
        if a == "-h" || a == "--help" {
            if stdout.write_all(b"Usage: numfmt [OPTION] [NUMBER]...\n").is_err() {
                return 1;
            }
            return 0;
        }
        if a == "-V" || a == "--version" {
            if stdout.write_all(b"numfmt (uutils coreutils) 0.10.0\n").is_err() {
                return 1;
            }
            return 0;
        }
        if a == "--from" || a == "--from-unit" || a == "--to" || a == "--to-unit" || a == "--format"
            || a == "--field" || a == "--padding" || a == "--delimiter" || a == "--suffix"
            || a == "--unit-separator" || a == "--round" || a == "--header" || a == "-d"
        {
            if i + 1 >= norm.len() {
                let _ = stderr.write_all(b"numfmt: missing operand\n");
                return 2;
            }
            let v = norm[i + 1].clone();
            i += 1;
            match a {
                "--from" | "--to" => {
                    let Some(u) = unit_of(&v) else {
                        let _ = stderr.write_all(b"numfmt: invalid unit\n");
                        return 2;
                    };
                    if a == "--from" {
                        from = u;
                    } else {
                        to = u;
                    }
                }
                "--from-unit" => from_size = v.parse::<f64>().unwrap_or(1.0),
                "--to-unit" => to_size = v.parse::<f64>().unwrap_or(1.0),
                "--format" => format = v,
                "--field" => {
                    fields.clear();
                    for part in v.split(',') {
                        if part == "-" {
                            fields.push((1, None));
                        } else if let Some((a2, b2)) = part.split_once('-') {
                            let start: usize = a2.parse().unwrap_or(1);
                            let end: Option<usize> = if b2.is_empty() { None } else { b2.parse().ok() };
                            fields.push((start, end));
                        } else if let Some(b2) = part.strip_prefix('-') {
                            fields.push((1, b2.parse().ok()));
                        } else {
                            let n = part.parse().unwrap_or(1);
                            fields.push((n, Some(n)));
                        }
                    }
                }
                "--padding" => padding = v.parse::<i64>().unwrap_or(0),
                "-d" | "--delimiter" => delimiter = v.chars().next().unwrap_or(' '),
                "--suffix" => suffix = v,
                "--unit-separator" => unit_sep = v,
                "--round" => {
                    let Some(r) = round_of(&v) else {
                        let _ = stderr.write_all(b"numfmt: invalid rounding method\n");
                        return 2;
                    };
                    round = r;
                }
                "--header" => header = v.parse::<usize>().unwrap_or(1),
                _ => {}
            }
            i += 1;
            continue;
        }
        match a {
            "--grouping" => grouping = true,
            "-z" | "--zero-terminated" => zero = true,
            "--debug" | "--invalid" | "--valid" => {
                if i + 1 < args.len() {
                    i += 1;
                }
            }
            _ => numbers.push(a.to_string()),
        }
        i += 1;
    }

    if numbers.is_empty() {
        let mut buf = Vec::new();
        if stdin.read_to_end(&mut buf).is_err() {
            return 1;
        }
        for chunk in buf.split(|c| *c == if zero { 0 } else { b'\n' }) {
            if !chunk.is_empty() {
                numbers.push(String::from_utf8_lossy(chunk).to_string());
            }
        }
    }

    for line in &numbers {
        if header > 0 {
            header -= 1;
            if stdout.write_all(format!("{line}\n").as_bytes()).is_err() {
                return 1;
            }
            continue;
        }
        let parts: Vec<String> = if delimiter == ' ' {
            line.split_whitespace().map(|s| s.to_string()).collect()
        } else {
            line.split(delimiter).map(|s| s.to_string()).collect()
        };
        let mut out_parts: Vec<String> = Vec::new();
        for (idx, part) in parts.iter().enumerate() {
            let field = idx + 1;
            let target = fields.iter().any(|(s, e)| field >= *s && (e.is_none() || field <= e.unwrap()));
            if !target {
                out_parts.push(part.clone());
                continue;
            }
            let Some(value) = parse_number(part, from, from_size, &suffix) else {
                let msg = format!("numfmt: invalid number: '{part}'\n");
                if stderr.write_all(msg.as_bytes()).is_err() {
                    return 1;
                }
                return 2;
            };
            let mut precision = numeric_body(part).split('.').nth(1).map(|f| f.len()).unwrap_or(0);
            let mut text;
            let mut grouped = grouping;
            let mut width = 0usize;
            let mut left = false;
            let mut zero_pad = false;
            let mut prec_set = false;
            if !format.is_empty() {
                if let Some(pos) = format.find('%') {
                    let rest = &format[pos + 1..];
                    let mut k = 0;
                    if rest.starts_with('\'') {
                        grouped = true;
                        k += 1;
                    }
                    if rest.starts_with('-') {
                        left = true;
                        k += 1;
                    }
                    let mut digits = String::new();
                    while k < rest.len() && rest[k..k + 1].chars().next().unwrap().is_ascii_digit() {
                        digits.push(rest.chars().nth(k).unwrap());
                        k += 1;
                    }
                    if !digits.is_empty() {
                        width = digits.parse().unwrap_or(0);
                        zero_pad = digits.starts_with('0');
                    }
                    if k < rest.len() && rest[k..k + 1].chars().next() == Some('.') {
                        k += 1;
                        let mut pd = String::new();
                        while k < rest.len() && rest[k..k + 1].chars().next().unwrap().is_ascii_digit() {
                            pd.push(rest.chars().nth(k).unwrap());
                            k += 1;
                        }
                        precision = pd.parse().unwrap_or(precision);
                        prec_set = true;
                    }
                    let conv = &rest[k..k + 1];
                    if conv == "d" || conv == "i" || conv == "u" {
                        precision = 0;
                    }
                }
            }
            if to == Unit::None {
                text = format_number(value, precision, grouped, round);
                if format.is_empty() && text.contains('.') {
                    while text.ends_with('0') {
                        text.pop();
                    }
                    if text.ends_with('.') {
                        text.pop();
                    }
                }
            } else {
                let base = to_size * 1024.0;
                let mut exp = 0usize;
                let mut scaled = value;
                if to == Unit::Si {
                    let b = to_size * 1000.0;
                    while scaled.abs() >= b && exp < SI_LABELS.len() - 1 {
                        scaled /= b;
                        exp += 1;
                    }
                } else {
                    while scaled.abs() >= base && exp < SI_LABELS.len() - 1 {
                        scaled /= base;
                        exp += 1;
                    }
                }
                if precision == 0 && !prec_set {
                    precision = 1;
                }
                text = format_number(scaled, precision, grouped, round);
                if format.is_empty() && text.contains('.') {
                    while text.ends_with('0') {
                        text.pop();
                    }
                    if text.ends_with('.') {
                        text.pop();
                    }
                }
                let lab = label(to, exp);
                if !lab.is_empty() {
                    text.push_str(&unit_sep);
                    text.push_str(&lab);
                }
            }
            if width > 0 && width > text.len() {
                let fill = width - text.len();
                if left {
                    text = format!("{}{}", text, " ".repeat(fill));
                } else if zero_pad {
                    text = format!("{}{}", "0".repeat(fill), text);
                } else {
                    text = format!("{}{}", " ".repeat(fill), text);
                }
            }
            if !suffix.is_empty() {
                text.push_str(&suffix);
            }
            out_parts.push(text);
        }
        let sep = if delimiter == ' ' { " ".to_string() } else { delimiter.to_string() };
        let joined = out_parts.join(&sep);
        let pad = padding;
        let rendered = if pad.unsigned_abs() as usize > joined.len() {
            let fill = pad.unsigned_abs() as usize - joined.len();
            if pad > 0 {
                format!("{}{}", " ".repeat(fill), joined)
            } else {
                format!("{}{}", joined, " ".repeat(fill))
            }
        } else {
            joined
        };
        if stdout.write_all(format!("{rendered}\n").as_bytes()).is_err() {
            return 1;
        }
    }
    0
}
