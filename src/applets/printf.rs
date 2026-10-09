//! `printf`：依 man page（docs/man/printf.1.gz）實作。
//!
//! man page 重點：
//! - 至少印一次 FORMAT，只要還有剩餘 ARGUMENT 就重複。
//! - 跳脫字列：\" \\ \a \b \c \e \f \n \r \t \v \NNN(1-3 八進) \xHH(1-2) \uHHHH \UHHHHHHHH、%%。
//! - \c = End-of-Input（停止輸出）。
//! - 轉換：%s %b %c %i/%d %u %x/%X %o %q %f/%F %e/%E %g/%G。
//! - 第一參數 = 最小寬度（負數=靠左）；第二參數依轉換型別（%s=最大長度、%d=最小寬度、%g=有效位數）。
//! - 數值參數前綴：0=八進、0x=十六進、'c=字元常數。
//!
//! 假設（記錄於 docs/assumptions.md）：
//! - 未知跳脫（如 \q）原樣輸出（保留反斜線）。
//! - 轉換錯誤 → stderr 訊息 + exit 1；寫入錯誤 → exit 1。
//! - 參數用盡時：數值轉換用 0，字串轉換用空字串。
//! - %q 用 POSIX 單引號語法，非可印字元轉成 $'\xNN'。

use crate::In;
use crate::Out;

struct Ctx<'a> {
    argv: &'a [String],
    pos: usize,
}

fn next_arg(c: &mut Ctx) -> String {
    if c.pos < c.argv.len() {
        let v = c.argv[c.pos].clone();
        c.pos += 1;
        v
    } else {
        String::new()
    }
}

fn next_number(c: &mut Ctx, stderr: Out) -> Result<i64, ()> {
    let raw = next_arg(c);
    parse_number(&raw, stderr)
}

fn parse_number(s: &str, stderr: Out) -> Result<i64, ()> {
    if let Some(rest) = s.strip_prefix('\'') {
        // 字元常數
        let ch = rest.chars().next();
        return match ch {
            Some(c) => Ok(c as i64),
            None => {
                let _ = stderr.write_all(b"printf: character constant expected\n");
                Err(())
            }
        };
    }
    let (sign, digits) = if s.starts_with('-') {
        (-1i64, &s[1..])
    } else {
        (1i64, s)
    };
    let radix: i64 = if digits.starts_with("0x") || digits.starts_with("0X") {
        16
    } else if digits.starts_with('0') && digits.len() > 1 {
        8
    } else {
        10
    };
    let body = if radix == 16 {
        digits[2..].to_string()
    } else {
        digits.to_string()
    };
    if body.is_empty() {
        let _ = stderr.write_all(b"printf: invalid number\n");
        return Err(());
    }
    let mut val: i64 = 0;
    for ch in body.chars() {
        let d = match ch.to_digit(radix as u32) {
            Some(d) => d as i64,
            None => {
                let _ = stderr.write_all(b"printf: invalid number\n");
                return Err(());
            }
        };
        val = val * radix + d;
    }
    Ok(sign * val)
}

fn escape_len(b: &[u8], i: usize) -> usize {
    let c = match b.get(i + 1) {
        Some(c) => *c,
        None => return 1,
    };
    match c {
        b'0'..=b'7' => {
            let mut n = 0;
            while n < 3 && i + 1 + n < b.len() && b[i + 1 + n] >= b'0' && b[i + 1 + n] <= b'7' {
                n += 1;
            }
            1 + n
        }
        b'x' => {
            let mut n = 0;
            while n < 2 && i + 2 + n < b.len() && (b[i + 2 + n] as char).is_ascii_hexdigit() {
                n += 1;
            }
            2 + n.max(1)
        }
        b'u' => 6,
        b'U' => 10,
        _ => 2,
    }
}

fn expand_escapes(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < b.len() {
        if b[i] != b'\\' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        if i + 1 >= b.len() {
            out.push(b'\\');
            break;
        }
        let c = b[i + 1];
        match c {
            b'"' => {
                out.push(b'"');
                i += 2;
            }
            b'\\' => {
                out.push(b'\\');
                i += 2;
            }
            b'a' => {
                out.push(7);
                i += 2;
            }
            b'b' => {
                out.push(8);
                i += 2;
            }
            b'c' => {
                out.push(3);
                i += 2;
            }
            b'e' => {
                out.push(27);
                i += 2;
            }
            b'f' => {
                out.push(12);
                i += 2;
            }
            b'n' => {
                out.push(10);
                i += 2;
            }
            b'r' => {
                out.push(13);
                i += 2;
            }
            b't' => {
                out.push(9);
                i += 2;
            }
            b'v' => {
                out.push(11);
                i += 2;
            }
            b'0'..=b'7' => {
                let mut v: u32 = 0;
                let mut n = 0;
                while n < 3 && i + 1 + n < b.len() && b[i + 1 + n] >= b'0' && b[i + 1 + n] <= b'7' {
                    v = v * 8 + (b[i + 1 + n] - b'0') as u32;
                    n += 1;
                }
                out.push(v as u8);
                i += 1 + n;
            }
            b'x' => {
                let mut v: u32 = 0;
                let mut n = 0;
                while n < 2 && i + 2 + n < b.len() {
                    let d = (b[i + 2 + n] as char).to_digit(16);
                    if d.is_none() {
                        break;
                    }
                    v = v * 16 + d.unwrap();
                    n += 1;
                }
                if n == 0 {
                    out.push(b'\\');
                    out.push(b'x');
                    i += 2;
                } else {
                    out.push(v as u8);
                    i += 2 + n;
                }
            }
            b'u' | b'U' => {
                let want = if c == b'u' { 4 } else { 8 };
                let mut v: u32 = 0;
                let mut n = 0;
                while n < want && i + 2 + n < b.len() {
                    let d = (b[i + 2 + n] as char).to_digit(16);
                    if d.is_none() {
                        break;
                    }
                    v = v * 16 + d.unwrap();
                    n += 1;
                }
                if n < want {
                    out.push(b'\\');
                    out.push(c);
                    i += 2;
                } else {
                    match char::from_u32(v) {
                        Some(ch) => {
                            let mut buf = [0u8; 4];
                            out.extend_from_slice(ch.encode_utf8(&mut buf).as_bytes());
                        }
                        None => out.push(b'?'),
                    }
                    i += 2 + n;
                }
            }
            _ => {
                out.push(b'\\');
                out.push(c);
                i += 2;
            }
        }
    }
    String::from_utf8_lossy(&out).to_string()
}

fn shell_quote(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }
    let bytes = s.as_bytes();
    let ok = bytes.iter().all(|b| {
        b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'/' | b'-' | b'+' | b'=' | b'@' | b'%' | b':')
    });
    if ok {
        return s.to_string();
    }
    let mut out = String::from("$'");
    for ch in s.chars() {
        if ch == '\'' {
            out.push_str("\\'");
        } else if (ch as u32) < 32 || (ch as u32) == 127 {
            out.push_str(&format!("\\x{:02x}", ch as u32));
        } else {
            out.push(ch);
        }
    }
    out.push('\'');
    out
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    if args.is_empty() {
        let _ = stderr.write_all(b"printf: missing format\n");
        return 1;
    }
    if args[0] == "--help" || args[0] == "-h" {
        let _ = stdout.write_all(b"Usage: printf FORMAT [ARGUMENT]...\n");
        return 0;
    }
    if args[0] == "--version" || args[0] == "-V" {
        let _ = stdout.write_all(b"printf (uutils coreutils) 0.10.0\n");
        return 0;
    }
    let fmt = args[0].clone();
    let argv = &args[1..];
    let mut c = Ctx { argv, pos: 0 };
    let b = fmt.as_bytes();
    let mut consumed_any = false;
    loop {
        let mut i = 0;
        while i < b.len() {
            let ch = b[i];
            if ch == b'\\' {
                if i + 1 < b.len() {
                    let c2 = b[i + 1];
                    if c2 == b'c' {
                        // End-of-Input
                        return 0;
                    }
                }
                let len = escape_len(b, i).min(b.len() - i);
                let s = expand_escapes(&fmt[i..i + len]);
                if stdout.write_all(s.as_bytes()).is_err() {
                    return 1;
                }
                i += len;
                continue;
            }
            if ch != b'%' {
                if stdout.write_all(&[ch]).is_err() {
                    return 1;
                }
                i += 1;
                continue;
            }
            // 轉換
            i += 1;
            let mut left = false;
            let mut plus = false;
            let mut space = false;
            let mut hash = false;
            let mut zero = false;
            while i < b.len() {
                match b[i] {
                    b'-' => left = true,
                    b'+' => plus = true,
                    b' ' => space = true,
                    b'#' => hash = true,
                    b'0' => zero = true,
                    _ => break,
                }
                i += 1;
            }
            let mut width: i64 = 0;
            if i < b.len() && b[i] == b'*' {
                width = match next_number(&mut c, stderr) {
                    Ok(v) => v,
                    Err(_) => return 1,
                };
                i += 1;
            } else {
                while i < b.len() && (b[i] as char).is_ascii_digit() {
                    width = width * 10 + (b[i] - b'0') as i64;
                    i += 1;
                }
            }
            let mut prec: i64 = -1;
            if i < b.len() && b[i] == b'.' {
                i += 1;
                if i < b.len() && b[i] == b'*' {
                    prec = match next_number(&mut c, stderr) {
                        Ok(v) => v,
                        Err(_) => return 1,
                    };
                    i += 1;
                } else {
                    prec = 0;
                    while i < b.len() && (b[i] as char).is_ascii_digit() {
                        prec = prec * 10 + (b[i] - b'0') as i64;
                        i += 1;
                    }
                }
            }
            if i >= b.len() {
                let _ = stderr.write_all(b"printf: invalid conversion specification\n");
                return 1;
            }
            let conv = b[i];
            i += 1;
            if conv == b'%' {
                if stdout.write_all(b"%").is_err() {
                    return 1;
                }
                continue;
            }
            let text = match conv {
                b's' | b'b' => {
                    let raw = next_arg(&mut c);
                    consumed_any = true;
                    let s = if conv == b'b' { expand_escapes(&raw) } else { raw };
                    let t = if prec >= 0 {
                        s.chars().take(prec as usize).collect::<String>()
                    } else {
                        s
                    };
                    t
                }
                b'c' => {
                    let raw = next_arg(&mut c);
                    consumed_any = true;
                    match raw.chars().next() {
                        Some(ch) => ch.to_string(),
                        None => "\0".to_string(),
                    }
                }
                b'q' => {
                    consumed_any = true;
                    shell_quote(&next_arg(&mut c))
                },
                b'd' | b'i' | b'u' | b'x' | b'X' | b'o' => {
                    let raw = next_arg(&mut c);
                    consumed_any = true;
                    let n = match parse_number(&raw, stderr) {
                        Ok(v) => v,
                        Err(_) => return 1,
                    };
                    let neg = n < 0;
                    let mag = if neg { -n } else { n };
                    let body = match conv {
                        b'd' | b'i' => mag.to_string(),
                        b'u' => mag.to_string(),
                        b'x' => format!("{:x}", mag),
                        b'X' => format!("{:X}", mag),
                        _ => format!("{:o}", mag),
                    };
                    let padded = if prec > 0 && prec > body.len() as i64 {
                        "0".repeat(prec as usize - body.len()) + &body
                    } else {
                        body
                    };
                    let prefix = if hash {
                        match conv {
                            b'x' => "0x",
                            b'X' => "0X",
                            b'o' => "0",
                            _ => "",
                        }
                    } else {
                        ""
                    };
                    let sign = if neg {
                        "-"
                    } else if plus {
                        "+"
                    } else if space && matches!(conv, b'd' | b'i') {
                        " "
                    } else {
                        ""
                    };
                    let core = format!("{}{}", prefix, padded);
                    let full = format!("{}{}", sign, core);
                    if zero && width > full.len() as i64 && !left {
                        let extra = "0".repeat(width as usize - full.len());
                        format!("{}{}{}", sign, prefix, extra + &padded)
                    } else {
                        full
                    }
                }
                b'f' | b'F' | b'e' | b'E' | b'g' | b'G' => {
                    let raw = next_arg(&mut c);
                    consumed_any = true;
                    let v: f64 = match raw.parse() {
                        Ok(v) => v,
                        Err(_) => {
                            let _ = stderr.write_all(b"printf: invalid number\n");
                            return 1;
                        }
                    };
                    let p = if prec < 0 { 6 } else { prec as usize };
                    let body = match conv {
                        b'f' | b'F' => format!("{:.1$}", v, p),
                        b'e' | b'E' => {
                            let s = format!("{:.1$e}", v, p);
                            let (m, e) = s.split_once('e').unwrap_or((s.as_str(), "0"));
                            let exp: i64 = e.parse().unwrap_or(0);
                            let body = format!("{}e{:+03}", m, exp);
                            if conv == b'E' {
                                body.to_uppercase()
                            } else {
                                body
                            }
                        }
                        b'g' | b'G' => {
                            if v == 0.0 {
                                "0".to_string()
                            } else {
                                let s = format!("{:.1$e}", v, p.saturating_sub(1));
                                let (mantissa, exp_str) = s.split_once('e').unwrap_or((s.as_str(), "0"));
                                let exp: i64 = exp_str.parse().unwrap_or(0);
                                let mantissa = mantissa.to_string();
                                if exp < -4 || exp >= p as i64 {
                                    if conv == b'G' {
                                        format!("{}E{:+03}", mantissa.to_uppercase(), exp)
                                    } else {
                                        format!("{}e{:+03}", mantissa, exp)
                                    }
                                } else {
                                    let trimmed = mantissa.trim_end_matches('0').trim_end_matches('.').to_string();
                                    if trimmed.is_empty() {
                                        "0".to_string()
                                    } else {
                                        trimmed
                                    }
                                }
                            }
                        }
                        _ => String::new(),
                    };
                    body
                }
                _ => {
                    let _ = stderr.write_all(b"printf: invalid conversion specification\n");
                    return 1;
                }
            };
            let text = if width > text.len() as i64 {
                let pad = " ".repeat(width as usize - text.len());
                if left {
                    format!("{}{}", text, pad)
                } else {
                    format!("{}{}", pad, text)
                }
            } else {
                text
            };
            if stdout.write_all(text.as_bytes()).is_err() {
                return 1;
            }
        }
        if consumed_any && c.pos < argv.len() {
            continue;
        }
        break;
    }
    0
}
