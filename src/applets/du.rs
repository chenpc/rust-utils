//! du(1) — Estimate file space usage.
//!
//! 依據 docs/man/du.1.gz（uutils EXTRA，含 --exclude 的 ?/*/{a,b} 語法）並與本機參考
//! uutils 0.10.0 逐 case 對 diff：
//! - 預設：1K 區塊（向上取整）、遞迴、硬連結只計一次（第二次出現完全不印）、symlink 不追蹤（計 0）
//! - `-a` 所有檔案、`-s`/`-d N` 深度、`-S` 不含子目錄、`-c` grand total
//! - `-A`/`-b` 表觀大小、`-B SIZE`/`-k`/`-m`/環境 DU_BLOCK_SIZE→BLOCK_SIZE→BLOCKSIZE
//! - `-h`/`--si` 人類可讀（一律一位小數，0 印 "0"）、`--inodes` 計條目數
//! - `-0` NUL 結尾、`-t SIZE` 閾值（**以位元組比較**）、`-x` 單一檔案系統、`-l` 硬連結重複計
//! - `-L`/`-D`/`-H`/`-P` symlink 追蹤、`--exclude PATTERN`/`-X FILE`（比對相對全路徑或 basename）
//! - `--files0-from=F`、`--time[=WORD]`（atime/ctime/status/birth/creation/use/access/mtime）、
//!   `--time-style full-iso|long-iso|iso`（預設 long-iso）
//! - 同一 (dev,ino) 已計過 → 整個條目不印也不貢獻（迴圈保護；operand `d d/e` 去重）
//! - operand 條目一律印（不受 -a 影響）；錯誤 "du: cannot access 'X': reason" / "du: cannot
//!   read directory 'X': reason" rc1 但繼續
//! - `-v` 為參考的 no-op 旗標（man：option not present in GNU）

use std::collections::HashSet;
use std::io::Write;
use std::os::unix::fs::MetadataExt;

use crate::pathname::io_reason_pub;
use crate::sys;
use crate::{In, Out};

const VERSION: &str = "du (uutils coreutils) 0.10.0";

#[derive(Default, PartialEq)]
enum Deref {
    #[default]
    None,
    Args,
    All,
}

#[derive(Default)]
struct Opt {
    all: bool,
    apparent: bool,
    unit: u64, // 0 = 未指定
    total: bool,
    max_depth: Option<usize>,
    human: u8, // 0/1(1024)/2(1000)
    inodes: bool,
    count_links: bool,
    deref: Deref,
    null: bool,
    separate: bool,
    summarize: bool,
    one_fs: bool,
    threshold: Option<i64>, // 位元組
    excludes: Vec<String>,
    time: Option<String>, // WORD
    time_style: String,
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut opt = Opt::default();
    let mut operands: Vec<String> = Vec::new();
    let mut files0_from: Option<String> = None;
    let mut conflict: Option<(String, String)> = None;

    let mut i = 0usize;
    while i < args.len() {
        let a = args[i].clone();
        i += 1;
        if a == "--" {
            operands.extend(args[i..].iter().cloned());
            break;
        }
        if !a.starts_with('-') || a == "-" {
            operands.push(a);
            continue;
        }
        if a.starts_with("--") {
            let (name, inline) = match a[2..].find('=') {
                Some(k) => (a[2..2 + k].to_string(), Some(a[2 + k + 1..].to_string())),
                None => (a[2..].to_string(), None),
            };
            macro_rules! val {
                () => {
                    match take_value(&name, inline.clone(), args, &mut i, stderr) {
                        Ok(v) => v,
                        Err(rc) => return rc,
                    }
                };
            }
            match name.as_str() {
                "all" => opt.all = true,
                "apparent-size" => opt.apparent = true,
                "bytes" => {
                    opt.apparent = true;
                    opt.unit = 1;
                }
                "total" => opt.total = true,
                "human-readable" => opt.human = 1,
                "si" => opt.human = 2,
                "inodes" => opt.inodes = true,
                "null" => opt.null = true,
                "separate-dirs" => opt.separate = true,
                "summarize" => {
                    if opt.all {
                        conflict = Some(("--all".into(), "--summarize".into()));
                    }
                    opt.summarize = true;
                }
                "one-file-system" => opt.one_fs = true,
                "verbose" => {}
                "dereference" => opt.deref = Deref::All,
                "dereference-args" => opt.deref = Deref::Args,
                "no-dereference" => opt.deref = Deref::None,
                "count-links" => opt.count_links = true,
                "block-size" => {
                    let v = val!();
                    match v.and_then(|v| parse_size(&v)) {
                        Some(n) => opt.unit = n,
                        None => {
                            writeln!(stderr, "du: invalid block size '{}'", inline.unwrap_or_default()).ok();
                            return 2;
                        }
                    }
                }
                "max-depth" => {
                    let v = val!();
                    match v.and_then(|v| v.parse::<usize>().ok()) {
                        Some(n) => opt.max_depth = Some(n),
                        None => {
                            writeln!(stderr, "du: invalid maximum depth '{}'", inline.unwrap_or_default()).ok();
                            return 2;
                        }
                    }
                }
                "threshold" => {
                    let v = val!();
                    match v.and_then(|v| parse_size_signed(&v)) {
                        Some(n) => opt.threshold = Some(n),
                        None => {
                            writeln!(stderr, "du: invalid threshold value '{}'", inline.unwrap_or_default()).ok();
                            return 2;
                        }
                    }
                }
                "exclude" => {
                    if let Some(v) = val!() {
                        opt.excludes.push(v);
                    }
                }
                "exclude-from" => {
                    if let Some(v) = val!() {
                        match std::fs::read_to_string(&v) {
                            Ok(text) => {
                                for line in text.lines() {
                                    if !line.is_empty() {
                                        opt.excludes.push(line.to_string());
                                    }
                                }
                            }
                            Err(e) => {
                                writeln!(stderr, "du: cannot access '{v}': {}", io_reason_pub(&e)).ok();
                                return 1;
                            }
                        }
                    }
                }
                "files0-from" => files0_from = val!(),
                "time" => opt.time = Some(inline.unwrap_or_else(|| "mtime".to_string())),
                "time-style" => {
                    if let Some(v) = val!() {
                        opt.time_style = v;
                    }
                }
                "help" => {
                    writeln!(stdout, "Usage: du [OPTION]... [FILE]...").ok();
                    writeln!(stdout, "Estimate file space usage").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "{VERSION}").ok();
                    return 0;
                }
                name => {
                    writeln!(stderr, "du: unrecognized option '--{name}'").ok();
                    return 2;
                }
            }
            continue;
        }
        let chars: Vec<char> = a[1..].chars().collect();
        let mut k = 0usize;
        macro_rules! attached_or_next {
            ($flag:expr) => {{
                let rest: String = chars[k + 1..].iter().collect();
                if !rest.is_empty() {
                    rest
                } else if i < args.len() {
                    let v = args[i].clone();
                    i += 1;
                    v
                } else {
                    writeln!(stderr, "du: option requires an argument -- '{}'", $flag).ok();
                    return 2;
                }
            }};
        }
        while k < chars.len() {
            match chars[k] {
                'a' => opt.all = true,
                'A' => opt.apparent = true,
                'b' => {
                    opt.apparent = true;
                    opt.unit = 1;
                }
                'c' => opt.total = true,
                'h' => opt.human = 1,
                'k' => opt.unit = 1024,
                'm' => opt.unit = 1024 * 1024,
                'l' => opt.count_links = true,
                'L' => opt.deref = Deref::All,
                'D' | 'H' => opt.deref = Deref::Args,
                'P' => opt.deref = Deref::None,
                'S' => opt.separate = true,
                's' => {
                    if opt.all {
                        conflict = Some(("--all".into(), "--summarize".into()));
                    }
                    opt.summarize = true;
                }
                'x' => opt.one_fs = true,
                '0' => opt.null = true,
                'v' => {}
                'B' => {
                    let v = attached_or_next!('B');
                    match parse_size(&v) {
                        Some(n) => opt.unit = n,
                        None => {
                            writeln!(stderr, "du: invalid block size '{v}'").ok();
                            return 2;
                        }
                    }
                    k = chars.len();
                    continue;
                }
                'd' => {
                    let v = attached_or_next!('d');
                    match v.parse::<usize>() {
                        Ok(n) => opt.max_depth = Some(n),
                        Err(_) => {
                            writeln!(stderr, "du: invalid maximum depth '{v}'").ok();
                            return 2;
                        }
                    }
                    k = chars.len();
                    continue;
                }
                't' => {
                    let v = attached_or_next!('t');
                    match parse_size_signed(&v) {
                        Some(n) => opt.threshold = Some(n),
                        None => {
                            writeln!(stderr, "du: invalid threshold value '{v}'").ok();
                            return 2;
                        }
                    }
                    k = chars.len();
                    continue;
                }
                'X' => {
                    let v = attached_or_next!('X');
                    match std::fs::read_to_string(&v) {
                        Ok(text) => {
                            for line in text.lines() {
                                if !line.is_empty() {
                                    opt.excludes.push(line.to_string());
                                }
                            }
                        }
                        Err(e) => {
                            writeln!(stderr, "du: cannot access '{v}': {}", io_reason_pub(&e)).ok();
                            return 1;
                        }
                    }
                    k = chars.len();
                    continue;
                }
                c => {
                    writeln!(stderr, "du: invalid option -- '{c}'").ok();
                    return 2;
                }
            }
            k += 1;
        }
    }

    if let Some((x, y)) = conflict {
        writeln!(stderr, "du: the argument '{x}' cannot be used with '{y}'").ok();
        return 1;
    }
    if opt.summarize {
        opt.max_depth = Some(0);
    } else if let Some(d) = opt.max_depth {
        if d == 0 {
            opt.max_depth = Some(0);
        }
    }

    // 區塊單位
    if opt.unit == 0 {
        for var in ["DU_BLOCK_SIZE", "BLOCK_SIZE", "BLOCKSIZE"] {
            if let Ok(v) = std::env::var(var) {
                if let Some(n) = parse_size(&v) {
                    opt.unit = n;
                    break;
                }
            }
        }
    }
    if opt.unit == 0 {
        opt.unit = if std::env::var_os("POSIXLY_CORRECT").is_some() { 512 } else { 1024 };
    }

    if let Some(f) = files0_from {
        let data = if f == "-" {
            let mut buf = Vec::new();
            use std::io::Read;
            let _ = stdin.take(64 * 1024 * 1024).read_to_end(&mut buf);
            buf
        } else {
            match std::fs::read(&f) {
                Ok(d) => d,
                Err(e) => {
                    writeln!(stderr, "du: cannot access '{f}': {}", io_reason_pub(&e)).ok();
                    return 1;
                }
            }
        };
        for name in data.split(|b| *b == 0) {
            if !name.is_empty() {
                operands.push(String::from_utf8_lossy(name).into_owned());
            }
        }
    }
    if operands.is_empty() {
        operands.push(".".to_string());
    }

    let mut w = Walker { opt: &opt, stdout, stderr, seen: HashSet::new(), grand_bytes: 0u64, grand_inodes: 0u64, status: 0 };
    for op in &operands {
        let path = std::path::Path::new(op);
        let m = match w.stat_top(path) {
            Some(m) => m,
            None => continue,
        };
        let key = (m.dev(), m.ino());
        if !w.seen.insert(key) {
            continue; // 已計過：整個 operand 略過
        }
        let (bytes, inodes) = w.walk(op, path, 0, &m, true);
        w.grand_bytes += bytes;
        w.grand_inodes += inodes;
    }
    if opt.total {
        if opt.inodes {
            let s = w.grand_inodes.to_string();
            emit(&mut *w.stdout, &opt, &s, "total", None);
        } else {
            let s = w.display_bytes(w.grand_bytes);
            emit(&mut *w.stdout, &opt, &s, "total", None);
        }
    }
    if w.status != 0 {
        return 1;
    }
    0
}

fn take_value(name: &str, inline: Option<String>, args: &[String], i: &mut usize, stderr: &mut dyn Write) -> Result<Option<String>, i32> {
    if let Some(v) = inline {
        return Ok(Some(v));
    }
    if *i < args.len() {
        let v = args[*i].clone();
        *i += 1;
        Ok(Some(v))
    } else {
        writeln!(stderr, "du: option requires an argument -- '{name}'").ok();
        Err(2)
    }
}

struct Walker<'a> {
    opt: &'a Opt,
    stdout: &'a mut dyn Write,
    stderr: &'a mut dyn Write,
    seen: HashSet<(u64, u64)>,
    grand_bytes: u64,
    grand_inodes: u64,
    status: i32,
}

impl<'a> Walker<'a> {
    fn follow(&self, top: bool) -> bool {
        match self.opt.deref {
            Deref::All => true,
            Deref::Args => top,
            Deref::None => false,
        }
    }
    fn stat_child(&self, path: &std::path::Path) -> Option<std::fs::Metadata> {
        if self.follow(false) {
            std::fs::metadata(path).ok()
        } else {
            std::fs::symlink_metadata(path).ok()
        }
    }
    fn stat_top(&mut self, path: &std::path::Path) -> Option<std::fs::Metadata> {
        let r = if self.follow(true) { std::fs::metadata(path) } else { std::fs::symlink_metadata(path) };
        match r {
            Ok(m) => Some(m),
            Err(e) => {
                writeln!(self.stderr, "du: cannot access '{}': {}", path.to_string_lossy(), io_reason_pub(&e)).ok();
                self.status = 1;
                None
            }
        }
    }

    fn own_bytes(&self, m: &std::fs::Metadata) -> u64 {
        if self.opt.apparent {
            m.size()
        } else {
            m.blocks() * 512
        }
    }

    fn display_bytes(&self, bytes: u64) -> String {
        if self.opt.human > 0 {
            humanize(bytes as u128, self.opt.human == 2)
        } else {
            ((bytes + self.opt.unit - 1) / self.opt.unit).to_string()
        }
    }

    /// 回傳 (對父層的位元組貢獻, inode 貢獻)。印行依規則。
    fn walk(&mut self, disp: &str, path: &std::path::Path, depth: usize, m: &std::fs::Metadata, top: bool) -> (u64, u64) {
        let own = self.own_bytes(m);
        let mut bytes = own;
        let mut inodes = 1u64;
        let is_dir = m.is_dir();
        if is_dir {
            match std::fs::read_dir(path) {
                Ok(rd) => {
                    for ent in rd.flatten() {
                        let fname = ent.file_name().to_string_lossy().to_string();
                        let cpath = ent.path();
                        let cdisp = format!("{disp}/{fname}");
                        let cm = match self.stat_child(&cpath) {
                            Some(cm) => cm,
                            None => {
                                writeln!(self.stderr, "du: cannot access '{cdisp}': No such file or directory").ok();
                                self.status = 1;
                                continue;
                            }
                        };
                        if self.opt.one_fs && cm.dev() != m.dev() {
                            continue;
                        }
                        if excluded(&self.opt.excludes, &cdisp, &fname) {
                            continue;
                        }
                        let key = (cm.dev(), cm.ino());
                        let dup_ok = self.opt.count_links && cm.nlink() > 1 && cm.is_file();
                        if !self.seen.insert(key) && !dup_ok {
                            continue; // 已計過（含迴圈保護）：不印不貢獻
                        }
                        let (cb, ci) = self.walk(&cdisp, &cpath, depth + 1, &cm, false);
                        bytes += cb;
                        inodes += ci;
                    }
                }
                Err(e) => {
                    writeln!(self.stderr, "du: cannot read directory '{disp}': {}", io_reason_pub(&e)).ok();
                    self.status = 1;
                }
            }
        }
        // 印行與貢獻
        let depth_ok = depth <= self.opt.max_depth.unwrap_or(usize::MAX);
        let show = if is_dir {
            depth_ok
        } else {
            (top || self.opt.all) && depth_ok
        };
        if show {
            let pass = match self.opt.threshold {
                None => true,
                Some(t) if t > 0 => bytes >= t as u64,
                Some(t) => bytes <= (-t) as u64,
            };
            if pass {
                let val = if self.opt.inodes {
                    if self.opt.separate && is_dir {
                        1u64
                    } else {
                        inodes
                    }
                } else if self.opt.separate && is_dir {
                    own
                } else {
                    bytes
                };
                let vs = if self.opt.inodes { val.to_string() } else { self.display_bytes(val) };
                let ts = self.opt.time.as_ref().map(|word| self.time_of(word, path, m, is_dir));
                emit(self.stdout, self.opt, &vs, disp, ts);
            }
        }
        let contrib_bytes = if self.opt.separate && is_dir { own } else { bytes };
        _ = top;
        (contrib_bytes, inodes)
    }

    /// 該條目（含子樹最大值）的時間字串。
    fn time_of(&self, word: &str, path: &std::path::Path, m: &std::fs::Metadata, is_dir: bool) -> String {
        let own_t = match word {
            "atime" | "access" | "use" => m.atime(),
            "ctime" | "status" => m.ctime(),
            "birth" | "creation" => m.ctime(), // 假設：無 statx，以 ctime 近似（見 assumptions.md）
            _ => m.mtime(),
        };
        let mut tmax = own_t;
        if is_dir {
            // 目錄取其內（含間接）最大時間：對 symlink 子項直接 lstat 避免跟隨
            if let Ok(rd) = std::fs::read_dir(path) {
                for ent in rd.flatten() {
                    if let Ok(cm) = std::fs::symlink_metadata(ent.path()) {
                        let ct = self.child_time(word, &cm);
                        if ct > tmax {
                            tmax = ct;
                        }
                    }
                }
            }
        }
        format_time(tmax, &self.opt.time_style)
    }
    fn child_time(&self, word: &str, cm: &std::fs::Metadata) -> i64 {
        match word {
            "atime" | "access" | "use" => cm.atime(),
            "ctime" | "status" => cm.ctime(),
            "birth" | "creation" => cm.ctime(),
            _ => cm.mtime(),
        }
    }
}

fn emit(out: &mut dyn Write, opt: &Opt, value: &str, name: &str, time: Option<String>) {
    match (&time, opt.null) {
        (Some(t), true) => {
            let _ = write!(out, "{value}\t{t}\t{name}\0");
        }
        (Some(t), false) => {
            let _ = writeln!(out, "{value}\t{t}\t{name}");
        }
        (None, true) => {
            let _ = write!(out, "{value}\t{name}\0");
        }
        (None, false) => {
            let _ = writeln!(out, "{value}\t{name}");
        }
    }
}

/// man EXTRA 的 PATTERN：?、*、{a,b}；比對相對全路徑或 basename。
fn excluded(patterns: &[String], path: &str, base: &str) -> bool {
    for p in patterns {
        if glob_match(p.as_str(), path) || glob_match(p.as_str(), base) {
            return true;
        }
    }
    false
}

/// 支援 ?、*、{a,b} 的 glob（递归回溯；巢狀 brace 支援一層）。
fn glob_match(pat: &str, text: &str) -> bool {
    glob_inner(pat.as_bytes(), text.as_bytes())
}

/// 以頂層 ',' 切分 brace 選項（不处理巢狀内的 ','：深度計數）。
fn split_commas(s: &[u8]) -> Vec<&[u8]> {
    let mut out = Vec::new();
    let mut start = 0usize;
    let mut depth = 0usize;
    for (idx, c) in s.iter().enumerate() {
        match c {
            b'{' => depth += 1,
            b'}' => depth = depth.saturating_sub(1),
            b',' if depth == 0 => {
                out.push(&s[start..idx]);
                start = idx + 1;
            }
            _ => {}
        }
    }
    out.push(&s[start..]);
    out
}

fn glob_inner(pat: &[u8], text: &[u8]) -> bool {
    if pat.is_empty() {
        return text.is_empty();
    }
    match pat[0] {
        b'?' => !text.is_empty() && glob_inner(&pat[1..], &text[1..]),
        b'*' => {
            for k in 0..=text.len() {
                if glob_inner(&pat[1..], &text[k..]) {
                    return true;
                }
            }
            false
        }
        b'{' => {
            // 找對應的 }
            let mut depth = 0usize;
            let mut end = None;
            for (idx, c) in pat.iter().enumerate() {
                match c {
                    b'{' => depth += 1,
                    b'}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(idx);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            let Some(end) = end else {
                // 沒有閉合：視為字面
                return !text.is_empty() && text[0] == b'{' && glob_inner(&pat[1..], &text[1..]);
            };
            let alts = split_commas(&pat[1..end]);
            for alt in alts {
                let mut combined = alt.to_vec();
                combined.extend_from_slice(&pat[end + 1..]);
                if glob_inner(&combined, text) {
                    return true;
                }
            }
            false
        }
        c => !text.is_empty() && text[0] == c && glob_inner(&pat[1..], &text[1..]),
    }
}

const UNITS1024: &[&str] = &["", "K", "M", "G", "T", "P", "E", "Z", "Y"];
const UNITS_SI: &[&str] = &["k", "M", "G", "T", "P", "E", "Z", "Y"];

/// du 的人類可讀（GNU 風格）：mantissa <10 一位小數（4096→"4.0K"）、>=10 整數（28672→"28K"）、0→"0"。所有精度 ceil。
fn humanize(bytes: u128, si: bool) -> String {
    if bytes == 0 {
        return "0".to_string();
    }
    let base: u128 = if si { 1000 } else { 1024 };
    let mut exp = 0usize;
    let mut scale = 1u128;
    while exp + 1 < UNITS1024.len() && bytes >= scale * base {
        scale *= base;
        exp += 1;
    }
    let suffix = if si { UNITS_SI[exp.saturating_sub(1)] } else { UNITS1024[exp] };
    if exp == 0 {
        return format!("{bytes}");
    }
    let tenth = (bytes * 10 + scale - 1) / scale; // 0.1 單位、ceil
    if tenth < 100 {
        format!("{}.{:01}{}", tenth / 10, tenth % 10, suffix)
    } else {
        format!("{}{}", (tenth + 9) / 10, suffix)
    }
}

fn format_time(secs: i64, style: &str) -> String {
    let tm = match sys::local_tm(secs) {
        Some(t) => t,
        None => return format!("{secs}"),
    };
    let year = tm.tm_year + 1900;
    let (mon, mday, hour, min, sec) = (tm.tm_mon + 1, tm.tm_mday, tm.tm_hour, tm.tm_min, tm.tm_sec);
    match style {
        "iso" => format!("{year:04}-{mon:02}-{mday:02}"),
        "full-iso" => {
            let off = tm.tm_gmtoff;
            let sign = if off < 0 { '-' } else { '+' };
            let a = off.abs();
            let offh = a / 3600;
            let offm = a % 3600 / 60;
            format!("{year:04}-{mon:02}-{mday:02} {hour:02}:{min:02}:{sec:02} {sign}{offh:02}{offm:02}")
        }
        _ => format!("{year:04}-{mon:02}-{mday:02} {hour:02}:{min:02}"), // long-iso / 預設
    }
}

fn parse_size(s: &str) -> Option<u64> {
    if s.is_empty() {
        return None;
    }
    let start = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    if start == 0 {
        return unit_size(s);
    }
    let num: u64 = s[..start].parse().ok()?;
    let unit = &s[start..];
    if unit.is_empty() {
        return Some(num);
    }
    unit_size(unit).and_then(|m| num.checked_mul(m))
}

fn unit_size(unit: &str) -> Option<u64> {
    Some(match unit {
        "k" => 1024, // 參考接受小寫 k（man 只列大寫；du --help 亦同）
        "K" | "KiB" => 1024,
        "M" | "MiB" => 1024u64.pow(2),
        "G" | "GiB" => 1024u64.pow(3),
        "T" | "TiB" => 1024u64.pow(4),
        "P" | "PiB" => 1024u64.pow(5),
        "E" | "EiB" => 1024u64.pow(6),
        "Z" | "ZiB" => 1024u64.pow(7),
        "Y" | "YiB" => 1024u64.pow(8),
        "KB" => 1000,
        "MB" => 1000u64.pow(2),
        "GB" => 1000u64.pow(3),
        "TB" => 1000u64.pow(4),
        "PB" => 1000u64.pow(5),
        "EB" => 1000u64.pow(6),
        "ZB" => 1000u64.pow(7),
        "YB" => 1000u64.pow(8),
        "B" => 1,
        _ => return None,
    })
}

fn parse_size_signed(s: &str) -> Option<i64> {
    if let Some(rest) = s.strip_prefix('-') {
        return Some(-(i64::try_from(parse_size(rest)?).ok()?));
    }
    Some(i64::try_from(parse_size(s)?).ok()?)
}
