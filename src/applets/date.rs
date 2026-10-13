//! date — 印出或設定系統日期與時間。
//!
//! 規格來源：`docs/man/date.1.gz`。man 未定義的部分我方決定（見 docs/assumptions.md「date」節）：
//! - 預設格式 `%a %b %e %H:%M:%S %Z %Y`（GNU 慣例形狀）。
//! - TZ 只支援 UTC/GMT 與固定偏移（`±HH`、`±HHMM`、`±HH:MM`）；其他名稱（無 tzdata）退回 UTC。
//! - `%Z`：offset 0 → `UTC`；否則 POSIX 式 `<±HH:MM>`。
//! - 解析支援：@epoch、YYYY-MM-DD[T ]HH:MM[:SS][.frac][Z|±offset]、HH:MM[:SS]（今天）、
//!   today/yesterday/tomorrow。無效字串 → `date: invalid date '<s>'`（rc 1）。
//! - 同時給 -d/-f/-r：最後一個生效。clap 錯誤 rc 2。
//! - --resolution 固定回 `0.000000001`（std 無法探測時鐘解析度）。

use std::os::unix::fs::MetadataExt;

use crate::pathname;
use crate::sys;
use crate::{In, Out};

const DATE_HELP: &str = r#"Print or set the system date and time

Usage: date [OPTION]... [FORMAT]

Options:
  -d, --date <STRING>             display time described by STRING, not 'now'
  -f, --file <DATEFILE>           like --date; once for each line of DATEFILE
  -I[<FMT>], --iso-8601[=<FMT>]  output date/time in ISO 8601 format. FMT='date' for date only (the default), 'hours', 'minutes', 'seconds', or 'ns' Example: 2006-08-14T02:34:56-06:00 [possible values: date, hours, minutes, seconds, ns]
      --resolution                output the available resolution of timestamps Example: 0.000000001
  -R, --rfc-email                 output date and time in RFC 5322 format. Example: Mon, 14 Aug 2006 02:34:56 -0600
      --rfc-3339 <FMT>            output date/time in RFC 3339 format. FMT='date', 'seconds', or 'ns' Example: 2006-08-14 02:34:56-06:00 [possible values: date, seconds, ns]
      --debug                     annotate the parsed date, and warn about questionable usage to stderr
  -r, --reference <FILE>          display the last modification time of FILE
  -s, --set <STRING>              set time described by STRING
  -u, --universal                 print or set Coordinated Universal Time (UTC)
      --format <FORMAT>           (GNU extension) use FORMAT to print the time; equivalent to a positional FORMAT
  -h, --help                      Print help
  -V, --version                   Print version

Arguments:
  [format]  
"#;

const DATE_LONGS: &[&str] = &[
    "date",
    "debug",
    "format",
    "file",
    "help",
    "iso-8601",
    "reference",
    "resolution",
    "rfc-3339",
    "rfc-email",
    "set",
    "universal",
    "version",
];

// ---- 日曆（公开算法，Howard Hinnant civil_from_days / days_from_civil）----
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}

fn civil_from_days(z: i64) -> (i64, i64, i64) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = z - era * 146097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    (if m <= 2 { y + 1 } else { y }, m, d)
}

fn weekday_of(z: i64) -> usize {
    (((z % 7) + 7 + 4) % 7) as usize // 0=Sun（1970-01-01 是 Thu）
}

const DOW_SHORT: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
const DOW_LONG: [&str; 7] = ["Sunday", "Monday", "Tuesday", "Wednesday", "Thursday", "Friday", "Saturday"];
const MON_SHORT: [&str; 12] = ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];
const MON_LONG: [&str; 12] = ["January", "February", "March", "April", "May", "June", "July", "August", "September", "October", "November", "December"];

#[derive(Clone, Copy)]
struct Bt {
    y: i64,
    mo: i64,
    d: i64,
    h: i64,
    mi: i64,
    s: i64,
    nsec: i64,
    zdays: i64,
    off: i32, // 顯示用 offset（秒）
}

fn bt_from(epoch: i64, nsec: i64, off: i32) -> Bt {
    let e = epoch + off as i64;
    let days = e.div_euclid(86400);
    let sod = e.rem_euclid(86400);
    let (y, mo, d) = civil_from_days(days);
    Bt { y, mo, d, h: sod / 3600, mi: (sod / 60) % 60, s: sod % 60, nsec, zdays: days, off }
}

fn local_offset(epoch: i64) -> i32 {
    match sys::local_tm(epoch) {
        Some(tm) => {
            let z = days_from_civil(tm.tm_year as i64 + 1900, tm.tm_mon as i64 + 1, tm.tm_mday as i64);
            let secs = z * 86400 + tm.tm_hour as i64 * 3600 + tm.tm_min as i64 * 60 + tm.tm_sec as i64;
            (secs - epoch) as i32
        }
        None => 0,
    }
}

/// 我方 TZ 規則：固定的 `TZ=±HH[:]MM` 優先於系統 local（靜態 binary 讀不到 tzdata）。
fn effective_local_offset(epoch: i64) -> i64 {
    match tz_offset_from_env() {
        Some(o) => o as i64,
        None => local_offset(epoch) as i64,
    }
}

fn tz_offset_from_env() -> Option<i32> {
    let tz = std::env::var("TZ").ok()?;
    parse_tz_spec(&tz)
}

/// "UTC"/"GMT" → 0；"±HH[:]MM"/"±HH" → 秒；其他名稱 → None（退回系統 local／UTC 由呼叫端決定）。
fn parse_tz_spec(s: &str) -> Option<i32> {
    if s == "UTC" || s == "GMT" {
        return Some(0);
    }
    let (sign, rest) = match s.strip_prefix('+') {
        Some(r) => (1i32, r),
        None => match s.strip_prefix('-') {
            Some(r) => (-1i32, r),
            None => return None,
        },
    };
    let digits: String = rest.chars().filter(|c| c.is_ascii_digit()).collect();
    if digits.len() != rest.replace(':', "").len() || digits.is_empty() {
        return None;
    }
    let (hh, mm) = match digits.len() {
        2 => (digits.parse::<i32>().ok()?, 0),
        3 | 4 => {
            let (a, b) = digits.split_at(digits.len() - 2);
            (a.parse::<i32>().ok()?, b.parse::<i32>().ok()?)
        }
        5 | 6 => {
            let (a, b) = digits.split_at(digits.len() - 4);
            (a.parse::<i32>().ok()?, b.parse::<i32>().ok()?)
        }
        _ => return None,
    };
    Some(sign * (hh * 3600 + mm * 60))
}

fn parse_offset_suffix(s: &str) -> Option<i32> {
    if s == "Z" || s.is_empty() {
        return Some(0);
    }
    parse_tz_spec(s)
}

struct Parsed {
    epoch: i64,
    nsec: i64,
    explicit_tz: Option<i32>,
}

fn parse_date(s: &str, utc_hint: bool, now: i64) -> Option<Parsed> {
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    if let Some(rest) = s.strip_prefix('@') {
        let e: i64 = rest.parse().ok()?;
        // @epoch 是絕對時刻：顯示偏移交給 TZ/-u/local（GNU 慣例）。
        return Some(Parsed { epoch: e, nsec: 0, explicit_tz: None });
    }
    match s {
        "today" => return Some(Parsed { epoch: now, nsec: 0, explicit_tz: None }),
        "yesterday" => return Some(Parsed { epoch: now - 86400, nsec: 0, explicit_tz: None }),
        "tomorrow" => return Some(Parsed { epoch: now + 86400, nsec: 0, explicit_tz: None }),
        _ => {}
    }
    // 拆出日期、時間、時區後綴
    let (body, tz_suffix) = split_tz_suffix(s);
    let mut date_part: Option<(i64, i64, i64)> = None;
    let mut time_part: Option<(i64, i64, i64, i64)> = None; // h, m, s, nsec

    let mut it = body.split(|c: char| c == 'T' || c == 't' || c == ' ');
    // 第一段可能是日期或時間
    let first = it.next()?;
    let second = it.next();
    if it.next().is_some() {
        return None; // 太多段
    }
    let (date_str, time_str): (&str, &str) = match second {
        Some(t) => (first, t),
        None => {
            if first.contains(':') {
                (".", first)
            } else {
                (first, "")
            }
        }
    };
    if date_str != "." {
        let mut dc = date_str.split('-');
        let (a, b, c) = (dc.next()?, dc.next()?, dc.next()?);
        if dc.next().is_some() {
            return None;
        }
        if a.len() != 4 || b.len() != 2 || c.len() != 2 {
            return None;
        }
        date_part = Some((a.parse().ok()?, b.parse().ok()?, c.parse().ok()?));
        if !(1..=12).contains(&date_part?.1) || !(1..=31).contains(&date_part?.2) {
            return None;
        }
    }
    if !time_str.is_empty() {
        let (hms, frac) = match time_str.split_once('.') {
            Some((a, b)) => (a, b),
            None => (time_str, ""),
        };
        let mut hc = hms.split(':');
        let hh: i64 = hc.next()?.parse().ok()?;
        let mm: i64 = hc.next()?.parse().ok()?;
        let ss: i64 = match hc.next() {
            Some(v) => v.parse().ok()?,
            None => 0,
        };
        if hc.next().is_some() {
            return None;
        }
        if !(0..=23).contains(&hh) || !(0..=59).contains(&mm) || !(0..=60).contains(&ss) {
            return None;
        }
        let mut nsec = 0i64;
        if !frac.is_empty() {
            if !frac.chars().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let mut f = frac.to_string();
            while f.len() < 9 {
                f.push('0');
            }
            nsec = f[..9].parse().ok()?;
        }
        time_part = Some((hh, mm, ss, nsec));
    }
    let tz = match tz_suffix {
        Some(t) => Some(parse_offset_suffix(t)?),
        None => None,
    };
    let (y, mo, d) = match date_part {
        Some(v) => v,
        None => {
            let off_base = if utc_hint || tz.is_some() { 0 } else { effective_local_offset(now) };
            civil_from_days((now + off_base).div_euclid(86400))
        }
    };
    let (hh, mm, ss, nsec) = time_part.unwrap_or((0, 0, 0, 0));
    let z = days_from_civil(y, mo, d);
    let epoch = z * 86400 + hh * 3600 + mm * 60 + ss;
    let explicit_tz = tz.or(if utc_hint { Some(0) } else { None });
    let epoch = match explicit_tz {
        Some(o) => epoch - o as i64,
        None => epoch - effective_local_offset(epoch),
    };
    Some(Parsed { epoch, nsec, explicit_tz })
}

/// 切出尾部時區後綴：Z / +HH[:]MM / -HH[:]MM / +HHMM / +HH。
fn split_tz_suffix(s: &str) -> (&str, Option<&str>) {
    let b = s.as_bytes();
    // 從尾巴找最後一個 +/-（在日期時間之後）或 Z
    if let Some(pos) = s.rfind('Z') {
        if pos + 1 == s.len() {
            return (&s[..pos], Some("Z"));
        }
    }
    // 找時間之後的符號：避免吃掉 "-YYYY"（日期負號不常見，但 -HH 必須在時間之後）
    let start = s.find(':').unwrap_or(usize::MAX);
    let mut i = s.len();
    while i > 0 {
        let idx = i - 1;
        let c = b[idx];
        if (c == b'+' || c == b'-') && idx > start {
            return (&s[..idx], Some(&s[idx..]));
        }
        i -= 1;
    }
    (s, None)
}

fn fmt_offset(off: i32, colon: bool) -> String {
    let sign = if off < 0 { '-' } else { '+' };
    let a = off.abs();
    if colon {
        format!("{}{:02}:{:02}", sign, a / 3600, (a % 3600) / 60)
    } else {
        format!("{}{:02}{:02}", sign, a / 3600, (a % 3600) / 60)
    }
}

fn iso_week(y: i64, mo: i64, d: i64) -> (i64, i64) {
    // 以「含 1/4 的那一週的星期四」為第一週基準（公開算法）。
    fn dow_iso(z: i64) -> i64 {
        let s = weekday_of(z) as i64; // 0=Sun
        if s == 0 { 7 } else { s } // 1=Mon..7=Sun
    }
    fn thu_of_jan4(year: i64) -> i64 {
        let j4 = days_from_civil(year, 1, 4);
        j4 + (4 - dow_iso(j4))
    }
    let z = days_from_civil(y, mo, d);
    let mut year = y;
    loop {
        let thu1 = thu_of_jan4(year);
        if z < thu1 {
            year -= 1;
            continue;
        }
        let thu_next = thu_of_jan4(year + 1);
        if z >= thu_next {
            year += 1;
            continue;
        }
        return (year, (z - thu1) / 7 + 1);
    }
}

// ---- strftime 子集 ----
fn strftime(fmt: &str, t: &Bt, epoch: i64) -> String {
    let mut out = String::new();
    let mut it = fmt.chars().peekable();
    while let Some(c) = it.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        // 修飾符：- _ 0 或寬度數字
        let mut flag = ' ';
        let mut width: Option<usize> = None;
        let mut digits = String::new();
        while let Some(&n) = it.peek() {
            if n == '-' || n == '_' || n == '0' {
                flag = n;
                it.next();
            } else if n.is_ascii_digit() {
                digits.push(n);
                it.next();
                if digits.len() >= 3 {
                    break;
                }
            } else {
                break;
            }
        }
        if !digits.is_empty() {
            width = digits.parse().ok();
        }
        let conv = match it.next() {
            Some(c) => c,
            None => {
                out.push('%');
                break;
            }
        };
        let doy = t.zdays - days_from_civil(t.y, 1, 1) + 1;
        // 每個轉換產出「未補齊字串」+ 預設寬度 + 預設補齊字元
        let (v0, minw, defpad): (String, usize, char) = match conv {
            'Y' => (format!("{}", t.y), 4, '0'),
            'y' => (format!("{:02}", t.y % 100), 2, '0'),
            'C' => (format!("{}", t.y / 100), 2, '0'),
            'm' => (format!("{}", t.mo), 2, '0'),
            'd' => (format!("{}", t.d), 2, '0'),
            'e' => (format!("{}", t.d), 2, ' '),
            'H' => (format!("{}", t.h), 2, '0'),
            'k' => (format!("{}", t.h), 2, ' '),
            'I' => (format!("{}", 1 + ((t.h + 11) % 12)), 2, '0'),
            'l' => (format!("{}", 1 + ((t.h + 11) % 12)), 2, ' '),
            'M' => (format!("{}", t.mi), 2, '0'),
            'S' => (format!("{}", t.s), 2, '0'),
            'j' => (format!("{}", doy), 3, '0'),
            'N' => {
                let prec = width.unwrap_or(9);
                let div = 10i64.pow(9 - prec.min(9) as u32);
                (format!("{:0width$}", t.nsec / div, width = prec), 0, '0')
            }
            's' => (format!("{}", epoch), 0, '0'),
            'u' => (format!("{}", if weekday_of(t.zdays) == 0 { 7 } else { weekday_of(t.zdays) }), 0, '0'),
            'w' => (format!("{}", weekday_of(t.zdays)), 0, '0'),
            'U' => {
                let jan1 = weekday_of(days_from_civil(t.y, 1, 1)) as i64;
                let w = (doy + 7 - jan1) / 7;
                (format!("{}", w), 2, '0')
            }
            'W' => {
                let jan1_mon = (weekday_of(days_from_civil(t.y, 1, 1)) as i64 + 6) % 7; // Mon=0
                let w = (doy - 1 + 7 - jan1_mon) / 7;
                (format!("{}", w), 2, '0')
            }
            'V' => {
                let (_, w) = iso_week(t.y, t.mo, t.d);
                (format!("{}", w), 2, '0')
            }
            'G' => {
                let (gy, _) = iso_week(t.y, t.mo, t.d);
                (format!("{}", gy), 4, '0')
            }
            'g' => {
                let (gy, _) = iso_week(t.y, t.mo, t.d);
                (format!("{:02}", gy % 100), 2, '0')
            }
            'a' => (DOW_SHORT[weekday_of(t.zdays)].to_string(), 0, ' '),
            'A' => (DOW_LONG[weekday_of(t.zdays)].to_string(), 0, ' '),
            'b' | 'h' => (MON_SHORT[(t.mo - 1) as usize % 12].to_string(), 0, ' '),
            'B' => (MON_LONG[(t.mo - 1) as usize % 12].to_string(), 0, ' '),
            'p' => (if t.h < 12 { "AM" } else { "PM" }.to_string(), 0, ' '),
            'P' => (if t.h < 12 { "am" } else { "pm" }.to_string(), 0, ' '),
            'z' => (fmt_offset(t.off, false), 0, ' '),
            'Z' => {
                if t.off == 0 {
                    ("UTC".to_string(), 0, ' ')
                } else {
                    (format!("<{}>", fmt_offset(t.off, true)), 0, ' ')
                }
            }
            'F' => (format!("{:04}-{:02}-{:02}", t.y, t.mo, t.d), 0, ' '),
            'D' => (format!("{:02}/{:02}/{:02}", t.mo, t.d, t.y % 100), 0, ' '),
            'T' => (format!("{:02}:{:02}:{:02}", t.h, t.mi, t.s), 0, ' '),
            'R' => (format!("{:02}:{:02}", t.h, t.mi), 0, ' '),
            'r' => (format!("{:02}:{:02}:{:02} {}", 1 + ((t.h + 11) % 12), t.mi, t.s, if t.h < 12 { "AM" } else { "PM" }), 0, ' '),
            'c' => (format!("{} {} {:>2} {:02}:{:02}:{:02} {:04}", DOW_SHORT[weekday_of(t.zdays)], MON_SHORT[(t.mo - 1) as usize % 12], t.d, t.h, t.mi, t.s, t.y), 0, ' '),
            'x' => (format!("{:02}/{:02}/{:02}", t.mo, t.d, t.y % 100), 0, ' '),
            'X' => (format!("{:02}:{:02}:{:02}", t.h, t.mi, t.s), 0, ' '),
            'n' => ("\n".to_string(), 0, ' '),
            't' => ("\t".to_string(), 0, ' '),
            '%' => ("%".to_string(), 0, ' '),
            other => (format!("%{}", other), 0, ' '),
        };
        // GNU 修飾符：'-' 不補齊、'_' 空格補齊、'0' 或預設補齊；數字是寬度。
        let numeric = minw > 0 || matches!(conv, 's' | 'u' | 'w' | 'N');
        let _ = numeric;
        let w = width.unwrap_or(minw);
        let n = v0.chars().count();
        if flag == '-' {
            out.push_str(&v0);
            let w = width.unwrap_or(0); // GNU '-'：無明示寬度就不補齊
            if n < w {
                out.push_str(&" ".repeat(w - n));
            }
        } else {
            let padc = if flag == '_' { ' ' } else { defpad };
            if n < w {
                out.push_str(&format!("{}{}", padc.to_string().repeat(w - n), v0));
            } else {
                out.push_str(&v0);
            }
        }
    }
    out
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut opt_date: Option<String> = None;
    let mut opt_file: Option<String> = None;
    let mut opt_ref: Option<String> = None;
    let mut opt_set: Option<String> = None;
    let mut iso: Option<(bool, String)> = None; // (is -I, fmt)
    let mut rfc3339: Option<String> = None;
    let mut rfc_email = false;
    let mut resolution = false;
    let mut debug = false;
    let mut utc = false;
    let mut format: Option<String> = None;
    let mut seen_long: Vec<&str> = Vec::new();
    let mut only_positional = false;
    let mut i = 0;

    macro_rules! dup {
        ($name:expr) => {{
            pathname::clap_dup($name, "", 2, stderr);
            return 2;
        }};
    }

    while i < args.len() {
        let a = args[i].clone();
        i += 1;
        if only_positional {
            if format.is_some() {
                return unexpected_pos(&a, stderr);
            }
            format = Some(a);
            continue;
        }
        if a == "--" {
            only_positional = true;
            continue;
        }
        if a.starts_with("--") {
            let name = a[2..].split('=').next().unwrap_or("").to_string();
            let inline = a.split_once('=').map(|(_, v)| v.to_string());
            let resolved = match pathname::resolve_long(&name, DATE_LONGS) {
                Ok(l) => l,
                Err(Some(c)) => {
                    write!(stderr, "error: unexpected argument '{}' found\n\n  tip: a similar argument exists: '--{}'\n\n", a, c).ok();
                    return usage_err(stderr);
                }
                Err(None) => return unexpected_arg(&a, stderr),
            };
            if seen_long.contains(&resolved) {
                dup!(resolved);
            }
            seen_long.push(resolved);
            match resolved {
                "help" => {
                    write!(stdout, "{}", DATE_HELP).ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "date (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                "universal" => utc = true,
                "debug" => debug = true,
                "resolution" => resolution = true,
                "rfc-email" => rfc_email = true,
                "rfc-3339" => {
                    let v = match take_value(&a, inline, args, &mut i) {
                        Some(v) => v,
                        None => {
                            pathname::clap_value_required("rfc-3339", "FMT", stderr);
                            return 2;
                        }
                    };
                    if !matches!(v.as_str(), "date" | "seconds" | "ns") {
                        writeln!(stderr, "error: invalid value '{}' for '--rfc-3339 <FMT>' [possible values: date, seconds, ns]", v).ok();
                        return 2;
                    }
                    rfc3339 = Some(v);
                }
                "iso-8601" => {
                    let v = inline.unwrap_or_else(|| "date".to_string());
                    if !matches!(v.as_str(), "date" | "hours" | "minutes" | "seconds" | "ns") {
                        writeln!(stderr, "error: invalid value '{}' for '--iso-8601 <FMT>' [possible values: date, hours, minutes, seconds, ns]", v).ok();
                        return 2;
                    }
                    iso = Some((true, v));
                }
                "format" => {
                    if format.is_some() {
                        dup!("format");
                    }
                    match take_value(&a, inline, args, &mut i) {
                        Some(v) => format = Some(v),
                        None => {
                            pathname::clap_value_required("format", "FORMAT", stderr);
                            return 2;
                        }
                    }
                }
                "date" => match take_value(&a, inline, args, &mut i) {
                    Some(v) => opt_date = Some(v),
                    None => {
                        pathname::clap_value_required("date", "STRING", stderr);
                        return 2;
                    }
                },
                "file" => match take_value(&a, inline, args, &mut i) {
                    Some(v) => opt_file = Some(v),
                    None => {
                        pathname::clap_value_required("file", "DATEFILE", stderr);
                        return 2;
                    }
                },
                "reference" => match take_value(&a, inline, args, &mut i) {
                    Some(v) => opt_ref = Some(v),
                    None => {
                        pathname::clap_value_required("reference", "FILE", stderr);
                        return 2;
                    }
                },
                "set" => match take_value(&a, inline, args, &mut i) {
                    Some(v) => opt_set = Some(v),
                    None => {
                        pathname::clap_value_required("set", "STRING", stderr);
                        return 2;
                    }
                },
                _ => return unexpected_arg(&a, stderr),
            }
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let chars: Vec<char> = a[1..].chars().collect();
            let mut k = 0;
            while k < chars.len() {
                match chars[k] {
                    'u' => utc = true,
                    'R' => rfc_email = true,
                    'h' => {
                        write!(stdout, "{}", DATE_HELP).ok();
                        return 0;
                    }
                    'V' => {
                        writeln!(stdout, "date (uutils coreutils) 0.10.0").ok();
                        return 0;
                    }
                    'I' => {
                        let mut rest: String = chars[k + 1..].iter().collect();
                        if rest.starts_with('=') {
                            rest.remove(0); // 支援 -I=hours 與 -Ihours
                        }
                        let v = if rest.is_empty() { "date".to_string() } else { rest };
                        if !matches!(v.as_str(), "date" | "hours" | "minutes" | "seconds" | "ns") {
                            writeln!(stderr, "error: invalid value '{}' for '--iso-8601 <FMT>' [possible values: date, hours, minutes, seconds, ns]", v).ok();
                            return 2;
                        }
                        iso = Some((true, v));
                        break;
                    }
                    'd' | 'f' | 'r' | 's' => {
                        let rest: String = chars[k + 1..].iter().collect();
                        let v = if !rest.is_empty() {
                            rest
                        } else if i < args.len() && !args[i].starts_with('-') {
                            let v = args[i].clone();
                            i += 1;
                            v
                        } else if i < args.len() {
                            let v = args[i].clone();
                            i += 1;
                            v
                        } else {
                            let long = match chars[k] {
                                'd' => "date",
                                'f' => "file",
                                'r' => "reference",
                                _ => "set",
                            };
                            let metavar = match chars[k] {
                                'd' => "STRING",
                                'f' => "DATEFILE",
                                'r' => "FILE",
                                _ => "STRING",
                            };
                            pathname::clap_value_required(long, metavar, stderr);
                            return 2;
                        };
                        match chars[k] {
                            'd' => opt_date = Some(v),
                            'f' => opt_file = Some(v),
                            'r' => opt_ref = Some(v),
                            _ => opt_set = Some(v),
                        }
                        break;
                    }
                    _ => {
                        write!(stderr, "error: unexpected argument '{}' found\n\n", pathname::clap_unknown_short(&a, k)).ok();
                        return usage_err(stderr);
                    }
                }
                k += 1;
            }
            continue;
        }
        if format.is_some() {
            return unexpected_pos(&a, stderr);
        }
        format = Some(a);
    }

    if resolution {
        writeln!(stdout, "0.000000001").ok();
        return 0;
    }

    if let Some(str_to_set) = &opt_set {
        let now = sys::now_epoch();
        let p = match parse_date(str_to_set, utc, now) {
            Some(p) => p,
            None => {
                writeln!(stderr, "date: invalid date '{}'", str_to_set).ok();
                return 1;
            }
        };
        #[repr(C)]
        struct TimeVal {
            sec: i64,
            usec: i64,
        }
        extern "C" {
            fn settimeofday(tv: *const TimeVal, tz: *const u8) -> i32;
        }
        let tv = TimeVal { sec: p.epoch, usec: 0 };
        let rc = unsafe { settimeofday(&tv as *const TimeVal, std::ptr::null()) };
        if rc != 0 {
            writeln!(stderr, "date: cannot set date: {}", pathname::io_reason_nosuffix(&std::io::Error::last_os_error())).ok();
            return 1;
        }
        return 0;
    }

    let now = sys::now_epoch();
    // 決定要印的時刻來源
    let mut sources: Vec<Parsed> = Vec::new();
    if let Some(f) = &opt_file {
        let text = match std::fs::read_to_string(f) {
            Ok(t) => t,
            Err(e) => {
                writeln!(stderr, "date: {}: {}", f, pathname::io_reason_nosuffix(&e)).ok();
                return 1;
            }
        };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            // 註解行（# 起頭）：跳過並記錄（我方決定；man 未定義 DATEFILE 格式）
            if line.starts_with('#') {
                if debug {
                    writeln!(stderr, "date: comment: {}", line).ok();
                }
                continue;
            }
            match parse_date(line, utc, now) {
                Some(p) => {
                    if debug {
                        writeln!(stderr, "date: parsed '{}' as epoch {}", line, p.epoch).ok();
                    }
                    sources.push(p);
                }
                None => {
                    writeln!(stderr, "date: invalid date '{}'", line).ok();
                    return 1;
                }
            }
        }
    }
    if let Some(d) = &opt_date {
        match parse_date(d, utc, now) {
            Some(p) => {
                if debug {
                    writeln!(stderr, "date: parsed '{}' as epoch {}", d, p.epoch).ok();
                }
                sources.push(p);
            }
            None => {
                writeln!(stderr, "date: invalid date '{}'", d).ok();
                return 1;
            }
        }
    }
    if let Some(r) = &opt_ref {
        match std::fs::metadata(r) {
            Ok(md) => sources.push(Parsed { epoch: md.mtime(), nsec: md.mtime_nsec() as i64, explicit_tz: None }),
            Err(e) => {
                writeln!(stderr, "date: {}: {}", r, pathname::io_reason_nosuffix(&e)).ok();
                return 1;
            }
        }
    }
    if sources.is_empty() {
        sources.push(Parsed { epoch: now, nsec: 0, explicit_tz: None });
    }

    for p in sources {
        let off: i32 = if utc {
            0
        } else if let Some(o) = p.explicit_tz {
            o
        } else if let Some(o) = tz_offset_from_env() {
            o
        } else {
            local_offset(p.epoch)
        };
        let t = bt_from(p.epoch, p.nsec, off);
        let line = if let Some(fmt) = &format {
            // 我方決定：positional FORMAT / --format 優先於 -I/-R/--rfc-3339
            strftime(fmt, &t, p.epoch)
        } else if let Some((_, fmt)) = &iso {
            let tzpart = fmt_offset(off, true);
            match fmt.as_str() {
                "date" => format!("{:04}-{:02}-{:02}", t.y, t.mo, t.d),
                "hours" => format!("{:04}-{:02}-{:02}T{:02}:00{}", t.y, t.mo, t.d, t.h, tzpart),
                "minutes" => format!("{:04}-{:02}-{:02}T{:02}:{:02}", t.y, t.mo, t.d, t.h, t.mi),
                "seconds" => format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}{}", t.y, t.mo, t.d, t.h, t.mi, t.s, tzpart),
                _ => format!("{:04}-{:02}-{:02}T{:02}:{:02}:{:02}.{:09}{}", t.y, t.mo, t.d, t.h, t.mi, t.s, t.nsec, tzpart),
            }
        } else if rfc3339.is_some() {
            let tzpart = fmt_offset(off, true);
            match rfc3339.as_deref().unwrap_or("seconds") {
                "date" => format!("{:04}-{:02}-{:02}", t.y, t.mo, t.d),
                "seconds" => format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}{}", t.y, t.mo, t.d, t.h, t.mi, t.s, tzpart),
                _ => format!("{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:09}{}", t.y, t.mo, t.d, t.h, t.mi, t.s, t.nsec, tzpart),
            }
        } else if rfc_email {
            format!(
                "{}, {:02} {} {:04} {:02}:{:02}:{:02} {}",
                DOW_SHORT[weekday_of(t.zdays)],
                t.d,
                MON_SHORT[(t.mo - 1) as usize % 12],
                t.y,
                t.h,
                t.mi,
                t.s,
                fmt_offset(off, false)
            )
        } else {
            strftime("%a %b %e %H:%M:%S %Z %Y", &t, p.epoch)
        };
        writeln!(stdout, "{}", line).ok();
    }
    0
}

fn take_value(_flag: &str, inline: Option<String>, args: &[String], i: &mut usize) -> Option<String> {
    if let Some(v) = inline {
        return Some(v);
    }
    if *i < args.len() {
        let v = args[*i].clone();
        *i += 1;
        return Some(v);
    }
    None
}

fn usage_err(stderr: Out) -> i32 {
    write!(
        stderr,
        "Usage: date [OPTION]... [FORMAT]\n\nFor more information, try '--help'.\n"
    )
    .ok();
    2
}

fn unexpected_arg(a: &str, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found\n\n", a).ok();
    usage_err(stderr);
    2
}

fn unexpected_pos(a: &str, stderr: Out) -> i32 {
    write!(stderr, "error: unexpected argument '{}' found\n\n", a).ok();
    write!(
        stderr,
        "  tip: to pass '{}' as a value, use '-- {}'\n\n",
        a, a
    )
    .ok();
    usage_err(stderr);
    2
}
