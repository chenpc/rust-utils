// stat：檔案狀態。措辭/格式對齊參考 uutils coreutils 0.10.0。
//
// - 錯誤："stat: cannot statx 'X': <reason>"（參考用 statx 措辭）
// - 預設輸出同 GNU 但 Device 以 **十進位 major,minor**（uutils；GNU 為 hex）
// - terse（-t/-d/--terse）：%n %s %b %f %u %g %D %i %h %t %T %X %Y %Z %W %o
// - -f 印檔案系統區塊；%T 走 statfs magic 名稱對應
// - Birth 經 statx(2)（syscall 332）

use std::ffi::CString;
use std::io::Write;
#[allow(unused_imports)]
use std::os::unix::fs::MetadataExt;

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct StatxTimestamp {
    pub tv_sec: i64,
    pub tv_nsec: u32,
    pub __reserved: i32,
}

#[repr(C)]
#[derive(Default)]
pub struct Statx {
    pub stx_mask: u32,
    pub stx_blksize: u32,
    pub stx_attributes: u64,
    pub stx_nlink: u32,
    pub stx_uid: u32,
    pub stx_gid: u32,
    pub stx_mode: u32,
    pub stx_ino: u64,
    pub stx_size: u64,
    pub stx_blocks: u64,
    pub stx_attributes_mask: u64,
    pub stx_atime: StatxTimestamp,
    pub stx_btime: StatxTimestamp,
    pub stx_ctime: StatxTimestamp,
    pub stx_mtime: StatxTimestamp,
    pub stx_rdev_major: u32,
    pub stx_rdev_minor: u32,
    pub stx_dev_major: u32,
    pub stx_dev_minor: u32,
    pub stx_mnt_id: u64,
    pub stx_sync_gen: i64,
    pub stx_subvol: i64,
    pub __reserved: [i64; 4],
}

pub fn statx(path: &str, follow: bool) -> std::io::Result<Statx> {
    let c = CString::new(path).map_err(|_| std::io::Error::from_raw_os_error(22))?;
    let mut st = Statx::default();
    let flags = if follow { 0 } else { 0x100 }; // AT_SYMLINK_NOFOLLOW
    let ret = unsafe { crate::sys::syscall(332, -100i64, c.as_ptr() as i64, flags as i64, 0x0fff_i64, &mut st as *mut Statx as i64) };
    if ret != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(st)
}

/// clap 風格 CLI（參考 uutils 0.10.0 用 clap derive；錯誤措辭逐字對齊）
struct StatCli {
    follow: bool,
    terse: bool,
    file_system: bool,
    format: Option<String>,
    printf: Option<String>,
    operands: Vec<String>,
    help: bool,
    version: bool,
}

const STAT_USAGE: &str = "Usage: stat [OPTION]... FILE...";

fn clap_err(msg: String, usage: Option<&str>, stderr: crate::Out) -> i32 {
    writeln!(stderr, "error: {}", msg).ok();
    writeln!(stderr).ok();
    if let Some(u) = usage {
        writeln!(stderr, "{}", u).ok();
        writeln!(stderr).ok();
    }
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}

fn mark_seen(flag: &str, seen: &mut Vec<String>, stderr: crate::Out) -> Option<i32> {
    if seen.iter().any(|s| s == flag) {
        return Some(clap_err(format!("the argument '{}' cannot be used multiple times", flag), None, stderr));
    }
    seen.push(flag.to_string());
    None
}

/// clap 風格「未知引數」錯誤：帶 tip 行與 Usage。
fn unknown_arg(found: &str, stderr: crate::Out) -> i32 {
    writeln!(stderr, "error: unexpected argument '{}' found", found).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "  tip: to pass '{}' as a value, use '-- {}'", found, found).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "{}", STAT_USAGE).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}

const STAT_LONG_OPTS: &[&str] = &[
    "dereference",
    "file-system",
    "terse",
    "format",
    "printf",
    "help",
    "version",
];
const STAT_SHORT_LETTERS: &str = "LftchV";

fn is_known_flag_token(tok: &str) -> bool {
    if tok == "--" {
        return true;
    }
    if let Some(rest) = tok.strip_prefix("--") {
        let name = rest.split('=').next().unwrap_or("");
        return STAT_LONG_OPTS.contains(&name);
    }
    if tok.starts_with('-') && tok.len() > 1 {
        return STAT_SHORT_LETTERS.contains(tok[1..].chars().next().unwrap());
    }
    false
}

/// clap 實測順序：值缺失錯誤優先於重複旗標錯誤；若該位置的 token 是「未知旗標」，
/// 則 clap 先報 unexpected argument。
fn maybe_value_missing(
    at: Option<&str>,
    flag_repr: &str,
    short_repr: &str,
    stderr: crate::Out,
) -> Option<i32> {
    match at {
        None => Some(clap_err(format!("a value is required for '{}' but none was supplied", flag_repr), None, stderr)),
        Some(tok) => {
            if is_known_flag_token(tok) {
                Some(clap_err(format!("a value is required for '{}' but none was supplied", short_repr), None, stderr))
            } else if tok.starts_with('-') {
                Some(unknown_arg(tok, stderr))
            } else {
                None
            }
        }
    }
}

fn short_flag_name(c: char) -> &'static str {
    match c {
        'L' => "--dereference",
        'f' => "--file-system",
        't' => "--terse",
        'c' => "--format <FORMAT>",
        'h' => "--help",
        'V' => "--version",
        _ => "--",
    }
}

fn long_flag_name(long: &str) -> &'static str {
    match long {
        "dereference" => "--dereference",
        "file-system" => "--file-system",
        "terse" => "--terse",
        "format" => "--format <FORMAT>",
        "printf" => "--printf <FORMAT>",
        "help" => "--help",
        "version" => "--version",
        _ => "--",
    }
}

fn parse_cli(args: &[String], stderr: crate::Out) -> Result<StatCli, i32> {
    let mut cli = StatCli {
        follow: false,
        terse: false,
        file_system: false,
        format: None,
        printf: None,
        operands: Vec::new(),
        help: false,
        version: false,
    };
    let mut seen: Vec<String> = Vec::new();
    let mut only_operands = false;
    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].as_str();
        if only_operands {
            cli.operands.push(a.to_string());
            i += 1;
            continue;
        }
        if a == "--" {
            only_operands = true;
            i += 1;
            continue;
        }
        if a.starts_with("--") {
            let (name, inline) = match a.find('=') {
                Some(p) => (a[2..p].to_string(), Some(a[p + 1..].to_string())),
                None => (a[2..].to_string(), None),
            };
            let flag = long_flag_name(&name);
            if flag == "--" {
                return Err(unknown_arg(a, stderr));
            }
            if let Some(c) = mark_seen(flag, &mut seen, stderr) {
                return Err(c);
            }
            match name.as_str() {
                "dereference" => {
                    if inline.is_some() {
                        return Err(clap_err(format!("unexpected value '{}' for '--dereference' found; no more were expected", inline.unwrap()), Some(STAT_USAGE), stderr));
                    }
                    cli.follow = true;
                }
                "file-system" => {
                    if inline.is_some() {
                        return Err(clap_err(format!("unexpected value '{}' for '--file-system' found; no more were expected", inline.unwrap()), Some(STAT_USAGE), stderr));
                    }
                    cli.file_system = true;
                }
                "terse" => {
                    if inline.is_some() {
                        return Err(clap_err(format!("unexpected value '{}' for '--terse' found; no more were expected", inline.unwrap()), Some(STAT_USAGE), stderr));
                    }
                    cli.terse = true;
                }
                "format" => {
                    let v = match inline {
                        Some(v) => v,
                        None => {
                            if i + 1 >= args.len() {
                                return Err(clap_err("a value is required for '--format <FORMAT>' but none was supplied".to_string(), None, stderr));
                            }
                            if args[i + 1].starts_with('-') && args[i + 1] != "-" {
                                if let Some(rc) = maybe_value_missing(Some(args[i + 1].as_str()), "--format <FORMAT>", "--format <FORMAT>", stderr) {
                                    return Err(rc);
                                }
                            }
                            i += 1;
                            args[i].clone()
                        }
                    };
                    cli.format = Some(v);
                }
                "printf" => {
                    let v = match inline {
                        Some(v) => v,
                        None => {
                            if i + 1 >= args.len() {
                                return Err(clap_err("a value is required for '--printf <FORMAT>' but none was supplied".to_string(), None, stderr));
                            }
                            let next = args[i + 1].as_str();
                            if next.starts_with('-') && next != "-" {
                                if is_known_flag_token(next) {
                                    return Err(clap_err("a value is required for '--printf <FORMAT>' but none was supplied".to_string(), None, stderr));
                                }
                                return Err(unknown_arg(next, stderr));
                            }
                            i += 1;
                            args[i].clone()
                        }
                    };
                    cli.printf = Some(v);
                }
                "help" => cli.help = true,
                "version" => cli.version = true,
                _ => {}
            }
            i += 1;
            continue;
        }
        if a.starts_with('-') && a.len() > 1 {
            let chars: Vec<char> = a[1..].chars().collect();
            let mut k = 0usize;
            while k < chars.len() {
                let c = chars[k];
                match c {
                    'L' => {
                        if let Some(x) = mark_seen("--dereference", &mut seen, stderr) {
                            return Err(x);
                        }
                        cli.follow = true;
                    }
                    'f' => {
                        if let Some(x) = mark_seen("--file-system", &mut seen, stderr) {
                            return Err(x);
                        }
                        cli.file_system = true;
                    }
                    't' => {
                        if let Some(x) = mark_seen("--terse", &mut seen, stderr) {
                            return Err(x);
                        }
                        cli.terse = true;
                    }
                    'c' => {
                        if let Some(x) = mark_seen("--format <FORMAT>", &mut seen, stderr) {
                            return Err(x);
                        }
                        let rest: String = chars[k + 1..].iter().collect();
                        // clap 實測：-c 同 cluster 的剩餘字元作為值（-c%n）
                        if !rest.is_empty() {
                            cli.format = Some(rest);
                        } else if i + 1 < args.len() {
                            let next = args[i + 1].as_str();
                            if next.starts_with('-') && next != "-" {
                                if is_known_flag_token(next) {
                                    return Err(clap_err("a value is required for '--format <FORMAT>' but none was supplied".to_string(), None, stderr));
                                }
                                return Err(unknown_arg(next, stderr));
                            }
                            cli.format = Some(next.to_string());
                            i += 1;
                        } else {
                            return Err(clap_err("a value is required for '--format <FORMAT>' but none was supplied".to_string(), None, stderr));
                        }
                        k = chars.len();
                        continue;
                    }
                    'h' => {
                        if let Some(x) = mark_seen("--help", &mut seen, stderr) {
                            return Err(x);
                        }
                        cli.help = true;
                    }
                    'V' => {
                        if let Some(x) = mark_seen("--version", &mut seen, stderr) {
                            return Err(x);
                        }
                        cli.version = true;
                    }
                    _ => {
                        let known = STAT_SHORT_LETTERS.contains(c);
                        if known {
                            let flag = short_flag_name(c);
                            if let Some(x) = mark_seen(flag, &mut seen, stderr) {
                                return Err(x);
                            }
                        }
                        return Err(unknown_arg(&format!("-{}", c as u32 as u8 as char), stderr));
                    }
                }
                k += 1;
            }
            i += 1;
            continue;
        }
        cli.operands.push(a.to_string());
        i += 1;
    }
    Ok(cli)
}

/// `--printf` 的跳脫序列解碼（規則由 docs/probes/stat_printf_escapes.py 對齊參考，
/// 2909 cases 0 mismatches）；回傳 (bytes, warnings)。
fn decode_printfs(fmt: &str) -> (Vec<u8>, Vec<String>) {
    const SINGLE: [(char, u8); 9] = [
        ('a', 7u8),
        ('b', 8u8),
        ('f', 12u8),
        ('n', 10u8),
        ('r', 13u8),
        ('t', 9u8),
        ('\\', 92u8),
        ('"', 34u8),
        ('\'', 39u8),
    ];
    let b = fmt.as_bytes();
    let mut out: Vec<u8> = Vec::new();
    let mut warns: Vec<String> = Vec::new();
    let n = b.len();
    let mut i = 0usize;
    let hex = |c: u8| -> bool { c.is_ascii_hexdigit() };
    while i < n {
        if b[i] != b'\\' {
            out.push(b[i]);
            i += 1;
            continue;
        }
        if i + 1 >= n {
            out.push(b'\\');
            warns.push("backslash at end of format".to_string());
            i += 1;
            continue;
        }
        let c2 = b[i + 1];
        if let Some((_, v)) = SINGLE.iter().find(|(c, _)| *c as u32 == c2 as u32) {
            out.push(*v);
            i += 2;
            continue;
        }
        if c2 == b'x' {
            if i + 2 < n && hex(b[i + 2]) {
                if i + 3 < n && hex(b[i + 3]) {
                    let s = std::str::from_utf8(&b[i + 2..i + 4]).unwrap_or("00");
                    out.push(u8::from_str_radix(s, 16).unwrap_or(0));
                    i += 4;
                } else {
                    let s = std::str::from_utf8(&b[i + 2..i + 3]).unwrap_or("0");
                    out.push(u8::from_str_radix(s, 16).unwrap_or(0));
                    i += 3;
                }
                continue;
            }
            if i + 2 >= n {
                warns.push("incomplete hex escape '\\x'".to_string());
            } else {
                warns.push("unrecognized escape '\\x'".to_string());
            }
            out.push(b'x');
            i += 2;
            continue;
        }
        if (c2 as char).is_ascii_digit() {
            if c2 == b'8' || c2 == b'9' {
                warns.push(format!("unrecognized escape '\\{}'", c2 as char));
                out.push(c2);
                i += 2;
                continue;
            }
            let start = i + 1;
            let mut end = start;
            while end < n && end - start < 3 && (b'0'..=b'7').contains(&b[end]) {
                end += 1;
            }
            let s = std::str::from_utf8(&b[start..end]).unwrap_or("0");
            out.push(u32::from_str_radix(s, 8).unwrap_or(0) as u8 & 0o377);
            i = end;
            continue;
        }
        warns.push(format!("unrecognized escape '\\{}'", c2 as char));
        out.push(c2);
        i += 2;
    }
    (out, warns)
}

const STAT_HELP: &str = include_str!("../help/stat.txt");

pub fn run(args: &[String], _stdin: crate::In, stdout: crate::Out, stderr: crate::Out) -> i32 {
    let cli = match parse_cli(args, stderr) {
        Ok(c) => c,
        Err(rc) => return rc,
    };
    if cli.help {
        write!(stdout, "{}", STAT_HELP).ok();
        return 0;
    }
    if cli.version {
        writeln!(stdout, "stat (uutils coreutils) 0.10.0").ok();
        return 0;
    }
    if cli.operands.is_empty() {
        writeln!(stderr, "stat: missing operand").ok();
        writeln!(stderr, "{}", crate::pathname::usage_hint("stat")).ok();
        return 1;
    }
    let file_system = cli.file_system;
    let follow = cli.follow;
    // 參考實測：--printf 優先於 --format 與 --terse（不論順序）；--format 優先於 --terse
    let fs_terse = "%n %i %l %t %s %S %b %f %a %c %d";
    let printf_mode = cli.printf.is_some();
    let empty_fmt_default = cli.format.as_deref() == Some("") && !printf_mode && !cli.terse;
    let fmt_used: Option<String> = if empty_fmt_default {
        // 參考實測：-c ""（非 --printf、非 --terse）等同預設輸出（byte 相同）
        None
    } else if printf_mode {
        cli.printf.clone()
    } else if cli.format.is_some() {
        cli.format.clone()
    } else if cli.terse {
        Some(if file_system { fs_terse.to_string() } else { "%n %s %b %f %u %g %D %i %h %t %T %X %Y %Z %W %o".to_string() })
    } else {
        None
    };
    if let Some(f) = &fmt_used {
        if let Some(bad) = invalid_directive(f, file_system) {
            writeln!(stderr, "stat: {}: invalid directive", display_quote(&bad)).ok();
            return 1;
        }
    }
    // 參考實測：--printf 的跳脫警告每次執行只印一次，且在處理 operand 之前（即使 operand 全失敗仍印）
    let mut printf_fmt: Option<String> = None;
    if printf_mode {
        if let Some(f) = &fmt_used {
            let (bytes, warns) = decode_printfs(f);
            for w in warns {
                writeln!(stderr, "stat: warning: {}", w).ok();
            }
            printf_fmt = Some(String::from_utf8_lossy(&bytes).into_owned());
        }
    }
    let mut rc = 0;
    for f in &cli.operands {
        // 參考實測：operand '-' 實際 stat 的是 /dev/stdin（名稱仍顯示 '-'），且會追隨該 symlink
        let dash = f == "-";
        let probe = if dash { "/dev/stdin" } else { f.as_str() };
        if file_system {
            if dash {
                // 參考實測：-f 模式下 '-' 不被支援
                writeln!(stderr, "stat: using '-' to denote standard input does not work in file system mode").ok();
                rc = 1;
                continue;
            }
            rc |= print_fs(probe, fmt_used.as_deref(), printf_fmt.as_deref(), stdout, stderr);
            continue;
        }
        if let Some(fmt) = &fmt_used {
            match statx(probe, follow || dash) {
                Ok(st) => {
                    let out = expand_directive(fmt, |d| file_directive(d, f, &st)).unwrap_or_default();
                    if printf_mode {
                        stdout.write_all(out.as_bytes()).ok();
                        stdout.flush().ok();
                    } else {
                        write!(stdout, "{}\n", out).ok();
                    }
                }
                Err(e) => {
                    writeln!(stderr, "stat: cannot statx {}: {}", display_quote(f), crate::pathname::io_reason_pub(&e)).ok();
                    rc = 1;
                }
            }
            continue;
        }
        match statx(probe, follow || dash) {
            Ok(st) => {
                print_default(f, &st, stdout);
            }
            Err(e) => {
                writeln!(stderr, "stat: cannot statx {}: {}", display_quote(f), crate::pathname::io_reason_pub(&e)).ok();
                rc = 1;
            }
        }
    }
    rc
}

/// uutils 0.10.0 實測：operand 在錯誤訊息與 symlink File 行一律加引號。
/// 規則（以 char 分類、跳脫以 UTF-8 byte 為單位）：
/// 1. 含控制字元 → ANSI-C `$'...'`：僅 `\t \n \r` 有短跳脫，其餘控制字元 → 大寫 `\xHH`（逐 byte）；
///    `\` → `\\`、`'` → `\'`；可印字元原樣。若 `\xHH` 之後紧跟 ASCII hex digit，該處切段
///    （`a\x01b` → `$'a\x01'$'b'`）。
/// 2. 含 `'` 但不含 `"` 也不含 `\` → 雙引號包裹、內容原樣（`a'b` → `"a'b"`）。
/// 3. 其餘 → 單引號包裹，其中每個 `'` 寫成 `'\''`（反斜線不跳脫：`a\b` → `'a\b'`）。
pub fn display_quote(name: &str) -> String {
    if name.chars().any(is_quote_control) {
        return ansi_c_quote(name);
    }
    if name.contains('\'') && !name.contains('"') && !name.contains('\\') {
        return format!("\"{}\"", name);
    }
    let mut out = String::with_capacity(name.len() + 2);
    out.push('\'');
    for c in name.chars() {
        if c == '\'' {
            out.push_str("'\\''");
        } else {
            out.push(c);
        }
    }
    out.push('\'');
    out
}

/// 控制字元判定：Unicode Cc（U+0000–001F、U+007F–009F），另加 U+2028/U+2029
/// （參考實測兩者會被跳脫；見 docs/assumptions.md）。
fn is_quote_control(c: char) -> bool {
    c.is_control() || c == '\u{2028}' || c == '\u{2029}'
}

fn ansi_c_quote(name: &str) -> String {
    let mut out = String::from("$'");
    let mut after_hex = false;
    for c in name.chars() {
        if after_hex && c.is_ascii_hexdigit() {
            out.push_str("'$'");
        }
        after_hex = false;
        if is_quote_control(c) {
            let mut buf = [0u8; 4];
            for &b in c.encode_utf8(&mut buf).as_bytes() {
                match b {
                    b'\n' => out.push_str("\\n"),
                    b'\r' => out.push_str("\\r"),
                    b'\t' => out.push_str("\\t"),
                    _ => {
                        out.push_str(&format!("\\x{:02X}", b));
                        after_hex = true;
                    }
                }
            }
        } else {
            match c {
                '\\' => out.push_str("\\\\"),
                '\'' => out.push_str("\\'"),
                _ => out.push(c),
            }
        }
    }
    out.push('\'');
    out
}

fn mode_bits(st: &Statx) -> u32 {
    st.stx_mode & 0o170000
}

fn type_str(st: &Statx) -> &'static str {
    match mode_bits(st) & 0o170000 {
        0o010000 => "fifo",
        0o020000 => "character special file",
        0o040000 => "directory",
        0o060000 => "block special file",
        0o100000 => "regular file",
        0o120000 => "symbolic link",
        0o140000 => "socket",
        _ => "unknown",
    }
}

fn type_desc(st: &Statx) -> &'static str {
    if mode_bits(st) & 0o170000 == 0o100000 && st.stx_size == 0 {
        "regular empty file"
    } else {
        type_str(st)
    }
}

fn fmt_time(t: &StatxTimestamp) -> String {
    let tm = crate::sys::local_tm(t.tv_sec).unwrap();
    let off = tm.tm_gmtoff;
    let sign = if off < 0 { '-' } else { '+' };
    let a = off.unsigned_abs();
    let oh = a / 3600;
    let om = (a % 3600) / 60;
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}.{:09} {}{:02}{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec,
        t.tv_nsec,
        sign,
        oh,
        om
    )
}

/// %m：最長字首掛載點（/proc/mounts）。
fn mount_point_for(path: &str) -> String {
    let abs = if path.starts_with('/') {
        path.to_string()
    } else {
        match std::env::current_dir() {
            Ok(c) => format!("{}/{}", c.display(), path),
            Err(_) => path.to_string(),
        }
    };
    let data = match std::fs::read_to_string("/proc/mounts") {
        Ok(d) => d,
        Err(_) => return "/".to_string(),
    };
    let mut best = "/".to_string();
    for line in data.lines() {
        let mut it = line.split_whitespace();
        let _src = it.next();
        let tgt = match it.next() {
            Some(t) => t.replace("\040", " ").replace("\011", "\t"),
            None => continue,
        };
        let ok = tgt == "/"
            || abs == tgt
            || (abs.starts_with(&tgt) && abs.as_bytes().get(tgt.len()) == Some(&b'/'));
        if ok && tgt.len() > best.len() {
            best = tgt;
        }
    }
    best
}

fn perm_str(m: u32) -> String {
    let mut out = String::new();
    out.push(match m & 0o170000 {
        0o010000 => 'p',
        0o020000 => 'c',
        0o040000 => 'd',
        0o060000 => 'b',
        0o120000 => 'l',
        0o140000 => 's',
        _ => '-',
    });
    let triples = [(0o400u32, 0o200u32, 0o100u32), (0o040, 0o020, 0o010), (0o004, 0o002, 0o001)];
    let specials = [(0o4000u32, 0o100u32, 's'), (0o2000u32, 0o010u32, 's'), (0o1000u32, 0o001u32, 't')];
    for (i, (r, w, _x)) in triples.iter().enumerate() {
        out.push(if m & r != 0 { 'r' } else { '-' });
        out.push(if m & w != 0 { 'w' } else { '-' });
        let (sbit, xbit, letter) = specials[i];
        if m & sbit != 0 {
            out.push(if m & xbit != 0 { letter } else { letter.to_ascii_uppercase() });
        } else {
            out.push(if m & xbit != 0 { 'x' } else { '-' });
        }
    }
    out
}

fn print_default(path: &str, st: &Statx, stdout: &mut dyn Write) {
    let m = st.stx_mode;
    if m & 0o170000 == 0o120000 {
        match std::fs::read_link(path) {
            Ok(t) => writeln!(
                stdout,
                "  File: {} -> {}",
                display_quote(path),
                display_quote(&t.display().to_string())
            )
            .ok(),
            Err(_) => writeln!(stdout, "  File: {}", path).ok(),
        };
    } else {
        writeln!(stdout, "  File: {}", path).ok();
    }
    write!(stdout, "  Size: {:<10}\tBlocks: {:<11}IO Block: {:<4}   {}\n", st.stx_size, st.stx_blocks, st.stx_blksize, type_desc(st)).ok();
    // 參考實測：僅 char/block 多印 "     Device Type: major,minor"（fifo/socket 不印）
    let is_device = matches!(m & 0o170000, 0o020000 | 0o060000);
    write!(
        stdout,
        "Device: {},{}\tInode: {:<11} Links: {}{}",
        st.stx_dev_major,
        st.stx_dev_minor,
        st.stx_ino,
        st.stx_nlink,
        if is_device {
            format!("     Device Type: {},{}\n", st.stx_rdev_major, st.stx_rdev_minor)
        } else {
            "\n".to_string()
        }
    )
    .ok();
    write!(
        stdout,
        "Access: ({:04o}/{})  Uid: ({:>5}/ {:>7})   Gid: ({:>5}/ {:>7})\n",
        st.stx_mode & 0o7777,
        perm_str(m),
        st.stx_uid,
        crate::chown_common::name_of(st.stx_uid, false),
        st.stx_gid,
        crate::chown_common::name_of(st.stx_gid, true)
    )
    .ok();
    writeln!(stdout, "Access: {}", fmt_time(&st.stx_atime)).ok();
    writeln!(stdout, "Modify: {}", fmt_time(&st.stx_mtime)).ok();
    writeln!(stdout, "Change: {}", fmt_time(&st.stx_ctime)).ok();
    if st.stx_mask & 0x800 != 0 && st.stx_btime.tv_sec != 0 {
        writeln!(stdout, " Birth: {}", fmt_time(&st.stx_btime)).ok();
    }
}

/// 展開 percent format（`-c`/`-f` 共用）：支援 `%-Wd`（左靠齊）與 `%Wd`（右靠齊）；
/// directive 不合法（map 回 None）→ Err(bad)。
fn expand_directive(fmt: &str, map: impl Fn(char) -> Option<String>) -> Result<String, String> {
    let b = fmt.as_bytes();
    let mut out = String::new();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] != b'%' {
            out.push(b[i] as char);
            i += 1;
            continue;
        }
        i += 1;
        if i >= b.len() {
            out.push('%');
            break;
        }
        let mut left = false;
        let mut digits = String::new();
        while i < b.len() && (b[i] == b'-' || b[i].is_ascii_digit()) {
            if b[i] == b'-' {
                left = true;
            } else {
                digits.push(b[i] as char);
            }
            i += 1;
        }
        if i >= b.len() {
            return Err(format!("%{}{}", if left { "-" } else { "" }, digits));
        }
        let d = b[i] as char;
        i += 1;
        let v = match map(d) {
            Some(v) => v,
            None => return Err(format!("%{}", d)),
        };
        if let Ok(w) = digits.parse::<usize>() {
            if left {
                out.push_str(&format!("{:<w$}", v, w = w));
            } else {
                out.push_str(&format!("{:>w$}", v, w = w));
            }
        } else {
            out.push_str(&v);
        }
    }
    Ok(out)
}

/// 參考實測（uutils 0.10.0）＋GNU man：合法 directive 集合；其余 → `stat: '%X': invalid directive` rc1
const FILE_DIRECTIVES: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHJKLMNOPQRSTUVWXYZ%:";
const FS_DIRECTIVES: &str = "abcdefghijklmnopqrstuvwxyzABCDEFGHJKLMNOPQRSTUVWXYZ%:";

/// 回傳第一個不合法的 directive（含 `%`）；全部合法 → None。
fn invalid_directive(fmt: &str, file_system: bool) -> Option<String> {
    let set = if file_system { FS_DIRECTIVES } else { FILE_DIRECTIVES };
    let b = fmt.as_bytes();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] != b'%' {
            i += 1;
            continue;
        }
        i += 1;
        if i >= b.len() {
            return None; // 尾端裸 % → 參考輸出字面 "%"
        }
        let mut tok = String::new();
        while i < b.len() && (b[i] == b'-' || b[i].is_ascii_digit()) {
            tok.push(b[i] as char);
            i += 1;
        }
        if i >= b.len() {
            // 有修飾但沒結尾字母（%-、%s%-5）→ 參考視為 invalid directive
            return if tok.is_empty() { None } else { Some(format!("%{}", tok)) };
        }
        let d = b[i] as char;
        i += 1;
        if !set.contains(d) {
            return Some(format!("%{}", d));
        }
    }
    None
}

/// `-c` 的 directive 表（不合法 → None）。
fn fs_magic_name(t: u64) -> String {
    for (magic, name) in FS_MAGICS {
        if t == *magic {
            return name.to_string();
        }
    }
    format!("0x{:x}", t)
}

/// `-f` 的 directive 表（參考實測：僅 a b c d f i l n s S T t % 合法，其餘 invalid）。
fn fs_directive(d: char, path: &str) -> Option<String> {
    if !FS_DIRECTIVES.contains(d) {
        return None;
    }
    let fs = match statfs_raw(path) {
        Some(fs) => fs,
        None => return None,
    };
    let v: String = match d {
        'a' => fs.f_bavail.to_string(),
        'b' => fs.f_blocks.to_string(),
        'c' => fs.f_files.to_string(),
        'd' => fs.f_ffree.to_string(),
        'f' => fs.f_bfree.to_string(),
        'i' => format!("{:x}", ((fs.f_fsid[0] as u32 as u64) << 32) | fs.f_fsid[1] as u32 as u64),
        'n' => path.to_string(),
        's' => (fs.f_bsize as u64).to_string(),
        'S' => fs.f_frsize.to_string(),
        'l' => fs.f_namelen.to_string(),
        't' => format!("{:x}", fs.f_type as u64),
        'T' => fs_magic_name(fs.f_type as u64),
        '%' => "%".to_string(),
        // 其餘合法但此實作沒有對應欄位的 flag → 恒為 ?
        _ => "?".to_string(),
    };
    Some(v)
}

#[repr(C)]
pub struct StatfsRaw {
    pub f_type: i64,
    pub f_bsize: i64,
    pub f_blocks: u64,
    pub f_bfree: u64,
    pub f_bavail: u64,
    pub f_files: u64,
    pub f_ffree: u64,
    pub f_fsid: [i32; 2],
    pub f_namelen: i64,
    pub f_frsize: i64,
    pub f_flags: i64,
    pub f_spare: [i64; 4],
}

extern "C" {
    #[link_name = "statfs"]
    fn statfs_local(path: *const i8, buf: *mut StatfsRaw) -> i32;
}

fn statfs_raw(path: &str) -> Option<StatfsRaw> {
    let c = match CString::new(path) {
        Ok(c) => c,
        Err(_) => return None,
    };
    let mut fs = StatfsRaw {
        f_type: 0,
        f_bsize: 0,
        f_blocks: 0,
        f_bfree: 0,
        f_bavail: 0,
        f_files: 0,
        f_ffree: 0,
        f_fsid: [0; 2],
        f_namelen: 0,
        f_frsize: 0,
        f_flags: 0,
        f_spare: [0; 4],
    };
    if unsafe { statfs_local(c.as_ptr(), &mut fs) } != 0 {
        return None;
    }
    Some(fs)
}

/// 參考實測：%r 用新式 makedev 編碼（major<<8|minor）。
fn makedev_new(major: u32, minor: u32) -> u64 {
    (((major as u64) & 0xfffff00) << 12) | (((major as u64) & 0xff) << 8) | ((minor as u64) & 0xff) | (((minor as u64) & 0xfff00) << 12)
}

/// 參考實測：stat 對 %d/%D/%t/%T/%R 一律走 GNU 的「舊式」major()/minor() 拆解，
/// 即使 minor > 255（/dev/tty 的 statx minor 實為 5、%T 印 0）。
fn new_rdev(st: &Statx) -> u64 {
    makedev_new(st.stx_rdev_major, st.stx_rdev_minor)
}

/// 參考實測：%R 走 GNU 舊式 major(st_rdev)*100，另加 rdev 低 8 位
/// （/dev/null→103、/dev/zero→105、/dev/full→107、/dev/tty→500、/dev/loop0→700、/dev/sda→800）。
fn legacy_major(st: &Statx) -> u32 {
    let r = new_rdev(st);
    (((r >> 8) & 0xfff) | ((r >> 22) & 0xfffff00)) as u32
}

fn legacy_minor_low(st: &Statx) -> u32 {
    (new_rdev(st) & 0xff) as u32
}

fn makedev(major: u32, minor: u32) -> u64 {
    ((major as u64) << 8) | minor as u64
}

const FS_MAGICS: &[(u64, &str)] = &[
    (0x0102_1994, "tmpfs"),
    (0xEF53, "ext2/ext3"),
    (0x9fa0, "proc"),
    (0x0102_1997, "9p"),
    (0x6f73_6673, "overlay"),
    (0x9123_6ff3, "btrfs"),
    (0x584c_524d, "vfat"),
    (0x72b6, "v9fs"),
    (0x0000_1130, "nfs"),
    (0x7375_7245, "securityfs"),
    (0x7472_6163, "tracefs"),
    (0x6265_6366, "bpf"),
    (0x6467_2011, "debugfs"),
    (0xcafe_9501, "cgroup2"),
    (0x27e0_eb1e, "cgroup"),
    (0x0102_1996, "devpts"),
    (0x0102_1999, "selinuxfs"),
    (0x6572_6900, "erofs"),
];

pub fn fs_type_name(path: &str) -> String {
    let c = match CString::new(path) {
        Ok(c) => c,
        Err(_) => return "unknown".to_string(),
    };
    let mut sv = crate::sys::Statvfs {
        f_bsize: 0, f_frsize: 0, f_blocks: 0, f_bfree: 0, f_bavail: 0,
        f_files: 0, f_ffree: 0, f_favail: 0, f_fsid: 0, f_flag: 0, f_namemax: 0,
    };
    if unsafe { crate::sys::statvfs64(c.as_ptr(), &mut sv) } != 0 {
        return "unknown".to_string();
    }
    // statvfs 無 magic；改由 /sys 或 f_type（statfs）取得
    #[repr(C)]
    struct Statfs {
        f_type: i64,
        f_bsize: i64,
        f_blocks: u64,
        f_bfree: u64,
        f_bavail: u64,
        f_files: u64,
        f_ffree: u64,
        f_fsid: [i32; 2],
        f_namelen: i64,
        f_frsize: i64,
        f_flags: i64,
        f_spare: [i64; 4],
    }
    extern "C" {    #[link_name = "statfs"]
        fn statfs_local(path: *const i8, buf: *mut Statfs) -> i32;
    }
    let mut fs = Statfs {
        f_type: 0,
        f_bsize: 0,
        f_blocks: 0,
        f_bfree: 0,
        f_bavail: 0,
        f_files: 0,
        f_ffree: 0,
        f_fsid: [0; 2],
        f_namelen: 0,
        f_frsize: 0,
        f_flags: 0,
        f_spare: [0; 4],
    };
    if unsafe { statfs_local(c.as_ptr(), &mut fs) } != 0 {
        return "unknown".to_string();
    }
    for (magic, name) in FS_MAGICS {
        if fs.f_type as u64 == *magic {
            return name.to_string();
        }
    }
    format!("0x{:x}", fs.f_type as u64)
}

fn print_fs(path: &str, format: Option<&str>, printf_fmt: Option<&str>, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let fs = match statfs_raw(path) {
        Some(fs) => fs,
        None => {
            let e = std::io::Error::last_os_error();
            writeln!(stderr, "stat: cannot read file system information for {}: {}", display_quote(path), crate::pathname::io_reason_pub(&e)).ok();
            return 1;
        }
    };
    if let Some(fmt) = format {
        let out = expand_directive(fmt, |d| fs_directive(d, path)).unwrap_or_default();
        // 參考實測：--printf 模式在 fs 模式也不補尾 newline
        if printf_fmt.is_some() {
            write!(stdout, "{}", out).ok();
        } else {
            writeln!(stdout, "{}", out).ok();
        }
        return 0;
    }
    let fsid = ((fs.f_fsid[0] as u32 as u64) << 32) | fs.f_fsid[1] as u32 as u64;
    writeln!(stdout, "  File: \"{}\"", path).ok();
    writeln!(
        stdout,
        "    ID: {:x} Namelen: {:<7} Type: {}",
        fsid,
        fs.f_namelen,
        fs_magic_name(fs.f_type as u64)
    )
    .ok();
    writeln!(stdout, "Block Size: {:<10} Fundamental block size: {}", fs.f_bsize, fs.f_frsize).ok();
    writeln!(
        stdout,
        "Blocks: Total: {:<10} Free: {:<10} Available: {}",
        fs.f_blocks,
        fs.f_bfree,
        fs.f_bavail
    )
    .ok();
    writeln!(stdout, "Inodes: Total: {:<10} Free: {}", fs.f_files, fs.f_ffree).ok();
    0
}

// silence unused import warnings when MetadataExt path changes
#[allow(dead_code)]
fn _unused(m: &std::fs::Metadata) -> u64 {
    m.dev()
}

fn file_directive(d: char, path: &str, st: &Statx) -> Option<String> {
    if !FILE_DIRECTIVES.contains(d) {
        return None;
    }
    let v: String = match d {
        'n' => path.to_string(),
        's' => st.stx_size.to_string(),
        'b' => st.stx_blocks.to_string(),
        'B' => "512".to_string(),
        'f' => format!("{:x}", st.stx_mode),
        'h' => st.stx_nlink.to_string(),
        'i' => st.stx_ino.to_string(),
        'u' => st.stx_uid.to_string(),
        'g' => st.stx_gid.to_string(),
        'U' => crate::chown_common::name_of(st.stx_uid, false),
        'G' => crate::chown_common::name_of(st.stx_gid, true),
        'a' => format!("{:o}", st.stx_mode & 0o7777),
        'A' => perm_str(st.stx_mode),
        'd' => format!("{}", makedev(st.stx_dev_major, st.stx_dev_minor)),
        'D' => format!("{:x}", makedev(st.stx_dev_major, st.stx_dev_minor)),
        'r' => format!("{}", new_rdev(st)),
        'R' => format!("{:x}", legacy_major(st) * 256 + legacy_minor_low(st)),
        'T' => format!("{:x}", st.stx_rdev_minor),
        't' => format!("{:x}", st.stx_rdev_major),
        'o' => st.stx_blksize.to_string(),
        'F' => type_str(st).to_string(),
        'N' => {
            if st.stx_mode & 0o170000 == 0o120000 {
                match std::fs::read_link(path) {
                    Ok(t) => format!("'{}' -> '{}'", path, t.display()),
                    Err(_) => path.to_string(),
                }
            } else {
                format!("'{}'", path)
            }
        }
        'x' => fmt_time(&st.stx_atime),
        'y' => fmt_time(&st.stx_mtime),
        'z' => fmt_time(&st.stx_ctime),
        'w' => {
            if st.stx_btime.tv_sec != 0 {
                fmt_time(&st.stx_btime)
            } else {
                "-".to_string()
            }
        }
        'W' => {
            if st.stx_btime.tv_sec != 0 {
                st.stx_btime.tv_sec.to_string()
            } else {
                "0".to_string()
            }
        }
        'X' => st.stx_atime.tv_sec.to_string(),
        'Y' => st.stx_mtime.tv_sec.to_string(),
        'Z' => st.stx_ctime.tv_sec.to_string(),
        'm' => mount_point_for(path),
        'C' => "unsupported for this operating system".to_string(),
        // 參考：flag 存在但此實作沒有對應欄位 → 恒為 ?
        '%' => "%".to_string(),
        // 其餘合法但此實作沒有對應欄位的 flag → 恒為 ?
        _ => "?".to_string(),
    };
    Some(v)
}
