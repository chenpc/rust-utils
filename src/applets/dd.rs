//! dd(1) — Copy, and optionally convert, a file system resource.
//!
//! 依據 docs/man/dd.1.gz（uutils EXTRA）與本機參考逐 case 對 diff（**跳過** EBCDIC 家族
//! ascii/ebcdic/ibm 與 conv=sparse，原因記錄於 docs/assumptions.md）：
//! - operands: bs/ibs/obs/cbs=BYTES、count=N、skip/seek(N|iseek/oseek)、if/of=FILE、
//!   conv=LIST、iflag/oflag=LIST、status=LEVEL
//! - conv: ucase/lcase（僅 ASCII 字母，GNU 語意）、block/unblock（需 cbs）、swab（跨記錄奇偶）、
//!   sync（NUL 補齊短讀）、noerror、notrunc、nocreat、excl、fsync、fdatasync
//! - iflag: count_bytes/skip_bytes；oflag: append/seek_bytes（其餘 general flags 接受但為 no-op）
//! - status: none/noxfer/progress（progress 每秒輸出進度到 stderr）
//! - 統計：`R+P records in` / `W+P records out` / `N bytes (SI, IEC) copied, T s, RATE/s`
//!   （N<512 時無括號；T 為 9 位小數去尾零）
//! - 錯誤：`dd: failed to open 'f': <reason>`、`dd: unrecognized operand 'x'`、
//!   `dd: invalid flag 'x'` rc1

use std::io::{Read, Seek, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::time::Instant;

use crate::pathname::io_reason_pub;
use crate::sys;
use crate::{In, Out};

const VERSION: &str = "dd (uutils coreutils) 0.10.0";

const O_EXCL: i32 = 0o200;

#[derive(Default)]
struct Cfg {
    bs: u64,
    ibs: u64,
    obs: u64,
    cbs: Option<u64>,
    count: Option<u64>,
    skip: u64,
    seek: u64,
    skip_bytes: bool,
    seek_bytes: bool,
    ifile: Option<String>,
    ofile: Option<String>,
    ucase: bool,
    lcase: bool,
    block: bool,
    unblock: bool,
    swab: bool,
    sync: bool,
    noerror: bool,
    notrunc: bool,
    nocreat: bool,
    excl: bool,
    fsync: bool,
    fdatasync: bool,
    append: bool,
    status: String,
    count_bytes: bool,
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut c = Cfg::default();
    for a in args {
        if a == "--help" || a == "-h" {
            writeln!(stdout, "Copy, and optionally convert, a file system resource").ok();
            writeln!(stdout, "\nUsage: dd [OPERAND]...").ok();
            return 0;
        }
        if a == "--version" || a == "-V" {
            writeln!(stdout, "{VERSION}").ok();
            return 0;
        }
        let Some((k, v)) = a.split_once('=') else {
            writeln!(stderr, "dd: Unrecognized operand '{a}'").ok();
            return 1;
        };
        match k {
            "bs" => match parse_num(v) {
                Some(n) if n > 0 => {
                    c.bs = n;
                    c.ibs = n;
                    c.obs = n;
                }
                _ => {
                    writeln!(stderr, "dd: invalid number '{v}'").ok();
                    return 1;
                }
            },
            "ibs" => match parse_num(v) {
                Some(n) if n > 0 => c.ibs = n,
                _ => {
                    writeln!(stderr, "dd: invalid number '{v}'").ok();
                    return 1;
                }
            },
            "obs" => match parse_num(v) {
                Some(n) if n > 0 => c.obs = n,
                _ => {
                    writeln!(stderr, "dd: invalid number '{v}'").ok();
                    return 1;
                }
            },
            "cbs" => match parse_num(v) {
                Some(n) if n > 0 => c.cbs = Some(n),
                _ => {
                    writeln!(stderr, "dd: invalid number '{v}'").ok();
                    return 1;
                }
            },
            "count" => match parse_num(v) {
                Some(n) => c.count = Some(n),
                None => {
                    writeln!(stderr, "dd: invalid number '{v}'").ok();
                    return 1;
                }
            },
            "skip" | "iseek" => match parse_num(v) {
                Some(n) => c.skip = n,
                None => {
                    writeln!(stderr, "dd: invalid number '{v}'").ok();
                    return 1;
                }
            },
            "seek" | "oseek" => match parse_num(v) {
                Some(n) => c.seek = n,
                None => {
                    writeln!(stderr, "dd: invalid number '{v}'").ok();
                    return 1;
                }
            },
            "if" => c.ifile = Some(v.to_string()),
            "of" => c.ofile = Some(v.to_string()),
            "conv" => {
                for f in v.split(',') {
                    match f {
                        "ucase" => c.ucase = true,
                        "lcase" => c.lcase = true,
                        "block" => c.block = true,
                        "unblock" => c.unblock = true,
                        "swab" => c.swab = true,
                        "sync" => c.sync = true,
                        "noerror" => c.noerror = true,
                        "notrunc" => c.notrunc = true,
                        "nocreat" => c.nocreat = true,
                        "excl" => c.excl = true,
                        "fsync" => c.fsync = true,
                        "fdatasync" => c.fdatasync = true,
                        "ascii" | "ebcdic" | "ibm" | "sparse" => {
                            writeln!(stderr, "dd: conversion '{f}' is not supported in this build").ok();
                            return 1;
                        }
                        f => {
                            writeln!(stderr, "dd: Unrecognized conv=CONV -> {f}").ok();
                            return 1;
                        }
                    }
                }
            }
            "iflag" => {
                for f in v.split(',') {
                    match f {
                        "count_bytes" => c.count_bytes = true,
                        "skip_bytes" => c.skip_bytes = true,
                        "skip" | "count" | "fullblock" | "buffers" | "nonblock" | "dsync" | "sync" | "noatime" | "nocache" | "nowait" | "directory" | "dio" => {}
                        f => {
                            writeln!(stderr, "dd: invalid input flag: ‘{f}’").ok();
                            return 1;
                        }
                    }
                }
            }
            "oflag" => {
                for f in v.split(',') {
                    match f {
                        "append" => c.append = true,
                        "seek_bytes" => c.seek_bytes = true,
                        "direct" | "dsync" | "sync" | "nonblock" | "noatime" | "nocache" | "nowait" | "directory" | "rsync" | "creat" | "nofollow" => {}
                        f => {
                            writeln!(stderr, "dd: invalid output flag: ‘{f}’").ok();
                            return 1;
                        }
                    }
                }
            }
            "status" => match v {
                "none" | "noxfer" | "progress" | "null" | "xfer" => c.status = v.to_string(),
                _ => {
                    writeln!(stderr, "dd: invalid status state '{v}'").ok();
                    return 1;
                }
            },
            _ => {
                writeln!(stderr, "dd: Unrecognized operand '{a}'").ok();
                return 1;
            }
        }
    }
    if c.ibs == 0 {
        c.ibs = 512;
    }
    if c.obs == 0 {
        c.obs = 512;
    }
    if (c.block || c.unblock) && c.cbs.is_none() {
        writeln!(stderr, "dd: conv=block or conv=unblock specified without cbs=N").ok();
        return 1;
    }
    if c.block && c.unblock {
        writeln!(stderr, "dd: conv=block and conv=unblock are not both supported").ok();
        return 1;
    }

    // 輸入
    let mut input: Box<dyn Read> = match &c.ifile {
        Some(f) => match std::fs::File::open(f) {
            Ok(mut file) => {
                if file.metadata().map(|m| m.is_dir()).unwrap_or(false) {
                    writeln!(stderr, "dd: failed to open '{f}': Is a directory").ok();
                    return 1;
                }
                if c.skip > 0 {
                    let off = if c.skip_bytes { c.skip } else { c.skip.saturating_mul(c.ibs) };
                    if file.seek(std::io::SeekFrom::Start(off)).is_err() {
                        // 不可 seek：讀掉
                        let mut disc = vec![0u8; 64 * 1024];
                        let mut left = off as usize;
                        while left > 0 {
                            let n = disc.len().min(left);
                            match file.read(&mut disc[..n]) {
                                Ok(0) => break,
                                Ok(k) => left -= k,
                                Err(_) => break,
                            }
                        }
                    }
                }
                Box::new(file)
            }
            Err(e) => {
                writeln!(stderr, "dd: failed to open '{f}': {}", io_reason_pub(&e)).ok();
                return 1;
            }
        },
        None => {
            // skip 對 stdin 只能讀掉
            let mut input: Box<dyn Read> = Box::new(stdin);
            if c.skip > 0 {
                let off = if c.skip_bytes { c.skip } else { c.skip.saturating_mul(c.ibs) };
                let mut disc = vec![0u8; 64 * 1024];
                let mut left = off as usize;
                while left > 0 {
                    let n = disc.len().min(left);
                    match input.read(&mut disc[..n]) {
                        Ok(0) => break,
                        Ok(k) => left -= k,
                        Err(_) => break,
                    }
                }
            }
            input
        }
    };

    // 輸出
    let mut output: Box<dyn Write> = match &c.ofile {
        Some(f) => {
            let mut oo = std::fs::OpenOptions::new();
            oo.write(true);
            if !c.nocreat {
                oo.create(true);
            }
            if c.excl {
                oo.custom_flags(O_EXCL);
            }
            if !c.notrunc && !c.append {
                oo.truncate(true);
            }
            if c.append {
                oo.append(true);
            }
            match oo.open(f) {
                Ok(mut file) => {
                    if c.seek > 0 && !c.append {
                        let off = if c.seek_bytes { c.seek } else { c.seek.saturating_mul(c.obs) };
                        let seekable = match file.seek(std::io::SeekFrom::Start(off)) {
                            Ok(_) => true,
                            Err(_) => {
                                // 可一般檔應可 seek；失敗（如管道）→寫 NUL 補齊
                                let zeros = vec![0u8; 64 * 1024];
                                let mut left = off as usize;
                                let mut ok = true;
                                while left > 0 {
                                    let n = zeros.len().min(left);
                                    if file.write_all(&zeros[..n]).is_err() {
                                        ok = false;
                                        break;
                                    }
                                    left -= n;
                                }
                                ok && left == 0
                            }
                        };
                        if !seekable {
                            writeln!(stderr, "dd: warning: 'seek' requested but output is not seekable").ok();
                        }
                    }
                    Box::new(file)
                }
                Err(e) => {
                    writeln!(stderr, "dd: failed to open '{f}': {}", io_reason_pub(&e)).ok();
                    return 1;
                }
            }
        }
        None => Box::new(stdout),
    };
    // 無 of 時的 seek：stdout fd 可 seek 則 lseek，否則（管道）寫 NUL 補齊（參考行為）
    if c.ofile.is_none() && c.seek > 0 && !c.append {
        let off = if c.seek_bytes { c.seek } else { c.seek.saturating_mul(c.obs) };
        if sys::lseek_pos(1, 0, 1).is_some() {
            let _ = sys::lseek_pos(1, off as i64, 0);
        } else {
            let zeros = vec![0u8; 64 * 1024];
            let mut left = off as usize;
            while left > 0 {
                let n = zeros.len().min(left);
                if output.write_all(&zeros[..n]).is_err() {
                    break;
                }
                left -= n;
            }
        }
    }

    let ibs = c.ibs as usize;
    let obs = c.obs as usize;
    let mut buf = vec![0u8; ibs];
    let mut rfull = 0u64;
    let mut rpart = 0u64;
    let mut wfull = 0u64;
    let mut wpart = 0u64;
    let mut total_out = 0u64;
    let mut status_rc = 0;

    // cbs 轉換的串流累積（block/unblock 以 cbs 記錄為單位作用於**輸入記錄**拼接後）
    let cbs = c.cbs.unwrap_or(0) as usize;
    let mut swab_carry: Option<u8> = None;
    let mut truncated = 0u64;
    let mut outbuf: Vec<u8> = Vec::new();
    let mut linebuf: Vec<u8> = Vec::new();
    let mut chunkbuf: Vec<u8> = Vec::new();
    let t0 = Instant::now();
    let mut last_progress = t0;
    let progress = c.status == "progress";

    let mut reads_done = 0u64;
    let mut bytes_done = 0u64;
    loop {
        match c.count {
            Some(n) if c.count_bytes => {
                if bytes_done >= n {
                    break;
                }
            }
            Some(n) => {
                if reads_done >= n {
                    break;
                }
            }
            None => {}
        }
        let want = match c.count {
            Some(n) if c.count_bytes && n.saturating_sub(bytes_done) < ibs as u64 => n.saturating_sub(bytes_done) as usize,
            _ => ibs,
        };
        match fill_read(&mut input, &mut buf[..want]) {
            ReadResult::Eof => {
                // cbs 轉換尾塊：EOF 時殘留部分線 / 不足 cbs 的殘塊也 flush（uutils 實測）
                if c.block && !linebuf.is_empty() {
                    let line = std::mem::take(&mut linebuf);
                    if line.len() > cbs {
                        truncated += 1;
                        outbuf.extend_from_slice(&line[..cbs]);
                    } else {
                        outbuf.extend_from_slice(&line);
                        outbuf.resize(outbuf.len() + (cbs - line.len()), b' ');
                    }
                }
                if c.unblock && !chunkbuf.is_empty() {
                    let rec = std::mem::take(&mut chunkbuf);
                    outbuf.extend_from_slice(&unblock_rec(&rec));
                }
                break;
            }
            ReadResult::Error(e) => {
                if c.noerror {
                    rpart += 1;
                    continue;
                }
                writeln!(stderr, "dd: error reading '{}': {}", c.ifile.as_deref().unwrap_or("standard input"), io_reason_pub(&e)).ok();
                status_rc = 1;
                break;
            }
            ReadResult::Got(mut n) => {
                reads_done += 1;
                if n == ibs {
                    rfull += 1;
                } else {
                    rpart += 1;
                    if c.sync {
                        // uutils 實測：補 NUL 至 ibs，但記錄計數仍屬 partial（0+1）
                        buf[n..ibs].fill(0);
                        n = ibs;
                    }
                }
                bytes_done += n as u64;
                let mut chunk: Vec<u8> = buf[..n].to_vec();
                if c.swab {
                    chunk = swab(&chunk, &mut swab_carry);
                }
                if c.ucase {
                    for b in chunk.iter_mut() {
                        if b.is_ascii_lowercase() {
                            *b = b.to_ascii_uppercase();
                        }
                    }
                }
                if c.lcase {
                    for b in chunk.iter_mut() {
                        if b.is_ascii_uppercase() {
                            *b = b.to_ascii_lowercase();
                        }
                    }
                }
                if c.block {
                    // uutils 0.10.0 實測：串流以 '\n' 切線，每線去尾 \n 後 len>cbs → 截斷+truncated++，
                    // 否則補空格至 cbs；EOF 時殘留部分線也flush（無尾 \n 也算一線）。
                    linebuf.extend_from_slice(&chunk);
                    while let Some(pos) = linebuf.iter().position(|b| *b == b'\n') {
                        let line: Vec<u8> = linebuf.drain(..=pos).collect();
                        let line = &line[..line.len() - 1];
                        if line.len() > cbs {
                            truncated += 1;
                            outbuf.extend_from_slice(&line[..cbs]);
                        } else {
                            outbuf.extend_from_slice(line);
                            outbuf.resize(outbuf.len() + (cbs - line.len()), b' ');
                        }
                    }
                } else if c.unblock {
                    // uutils 實測：串流以 cbs 切塊，每塊去尾端空格、加一個 \n；EOF 殘塊也 flush。
                    chunkbuf.extend_from_slice(&chunk);
                    while chunkbuf.len() >= cbs {
                        let rec: Vec<u8> = chunkbuf.drain(..cbs).collect();
                        outbuf.extend_from_slice(&unblock_rec(&rec));
                    }
                } else {
                    outbuf.extend_from_slice(&chunk);
                }
                // 寫出 obs 完整塊
                while outbuf.len() >= obs {
                    let rec: Vec<u8> = outbuf.drain(..obs).collect();
                    if let Err(e) = output.write_all(&rec) {
                        write_fail(&mut status_rc, &c, &c.ofile, e, stderr);
                        break;
                    }
                    wfull += 1;
                    total_out += obs as u64;
                }
                if progress && last_progress.elapsed().as_secs() >= 1 {
                    let t = t0.elapsed().as_secs_f64();
                    writeln!(stderr, "{} bytes ({}, {}) copied, {:.9} s, {}/s", total_out, si(total_out), iec(total_out), t, rate(total_out, t)).ok();
                    last_progress = Instant::now();
                }
            }
        }
    }
    // 尾巴：swab 奇數殘留直接吐出（參考實測：3B 輸入 "ab\n" → 輸出 "ba\n"，不補 NUL）
    if let Some(b) = swab_carry.take() {
        outbuf.push(b);
    }
    if !outbuf.is_empty() {
        if let Err(e) = output.write_all(&outbuf) {
            write_fail(&mut status_rc, &c, &c.ofile, e, stderr);
        } else {
            wpart += 1;
            total_out += outbuf.len() as u64;
        }
        outbuf.clear();
    }
    if c.fsync || c.fdatasync {
        if let Some(f) = &c.ofile {
            if let Ok(file) = std::fs::OpenOptions::new().write(true).open(f) {
                let r = if c.fsync { file.sync_all() } else { file.sync_data() };
                if let Err(e) = r {
                    writeln!(stderr, "dd: failed to sync '{}': {}", f, io_reason_pub(&e)).ok();
                    status_rc = 1;
                }
            }
        }
    }
    let _ = output.flush();
    let t = t0.elapsed().as_secs_f64();
    if c.status != "none" {
        writeln!(stderr, "{rfull}+{rpart} records in").ok();
        writeln!(stderr, "{wfull}+{wpart} records out").ok();
        if truncated > 0 {
            if truncated == 1 {
                writeln!(stderr, "1 truncated record").ok();
            } else {
                writeln!(stderr, "{truncated} truncated records").ok();
            }
        }
        if c.status != "noxfer" {
            if total_out < 512 {
                writeln!(stderr, "{total_out} bytes copied, {t:.9} s, {}/s", rate(total_out, t)).ok();
            } else {
                writeln!(stderr, "{total_out} bytes ({}, {}) copied, {t:.9} s, {}/s", si(total_out), iec(total_out), rate(total_out, t)).ok();
            }
        }
    }
    status_rc
}

enum ReadResult {
    Eof,
    Got(usize),
    Error(std::io::Error),
}

fn fill_read<R: Read>(r: &mut R, buf: &mut [u8]) -> ReadResult {
    let mut filled = 0usize;
    while filled < buf.len() {
        match r.read(&mut buf[filled..]) {
            Ok(0) => break,
            Ok(n) => filled += n,
            Err(e) if e.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(e) => return ReadResult::Error(e),
        }
    }
    if filled == 0 {
        ReadResult::Eof
    } else {
        ReadResult::Got(filled)
    }
}

fn swab(data: &[u8], carry: &mut Option<u8>) -> Vec<u8> {
    let mut out = Vec::with_capacity(data.len());
    let mut src: Vec<u8> = Vec::with_capacity(data.len() + 1);
    if let Some(b) = carry.take() {
        src.push(b);
    }
    src.extend_from_slice(data);
    let mut i = 0usize;
    while i + 1 < src.len() {
        out.push(src[i + 1]);
        out.push(src[i]);
        i += 2;
    }
    if i < src.len() {
        *carry = Some(src[i]);
    }
    out
}

fn unblock_rec(rec: &[u8]) -> Vec<u8> {
    let mut out = rec.to_vec();
    while out.last() == Some(&b' ' ) {
        out.pop();
    }
    out.push(b'\n');
    out
}

fn write_fail(rc: &mut i32, c: &Cfg, ofile: &Option<String>, e: std::io::Error, stderr: &mut dyn Write) {
    writeln!(stderr, "dd: writing to '{}': {}", ofile.as_deref().unwrap_or("standard output"), io_reason_pub(&e)).ok();
    if !c.noerror {
        *rc = 1;
    }
}

fn parse_num(s: &str) -> Option<u64> {
    if s.is_empty() {
        return None;
    }
    // 支援 xY 乘式（GNU：64xY=64*Y）
    if let Some((a, b)) = s.split_once('x') {
        return parse_num(a)?.checked_mul(parse_num(b)?);
    }
    let end = s.find(|c: char| !c.is_ascii_digit()).unwrap_or(s.len());
    let num: u64 = s[..end].parse().ok()?;
    let unit = &s[end..];
    if unit.is_empty() {
        return Some(num);
    }
    let m = match unit {
        "kB" => 1000,
        "KB" | "K" | "kiB" | "KiB" => 1024,
        "MB" | "MiB" => 1024u64.pow(2),
        "M" => 1024u64.pow(2),
        "GB" | "GiB" | "G" => 1024u64.pow(3),
        "TB" | "TiB" | "T" => 1024u64.pow(4),
        "PB" | "PiB" | "P" => 1024u64.pow(5),
        "EB" | "EiB" | "E" => 1024u64.pow(6),
        "YB" | "YiB" | "Y" => 1024u64.pow(8),
        "ZB" | "ZiB" | "Z" => 1024u64.pow(7),
        _ => return None,
    };
    num.checked_mul(m)
}

const SIU: &[&str] = &["B", "kB", "MB", "GB", "TB", "PB", "EB", "ZB", "YB"];
const IECU: &[&str] = &["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB", "ZiB", "YiB"];

fn human(n: u64, units: &[&str], base: u64) -> String {
    let mut exp = 0usize;
    let mut scale = 1u64;
    while exp + 1 < units.len() && n >= scale.saturating_mul(base) && (n as f64) / (scale as f64) >= base as f64 {
        scale *= base;
        exp += 1;
    }
    if exp == 0 {
        return format!("{n} {}", units[0]);
    }
    let tenth = (n as u128 * 10 + scale as u128 - 1) / scale as u128;
    format!("{}.{:01} {}", tenth / 10, tenth % 10, units[exp])
}

fn si(n: u64) -> String {
    human(n, SIU, 1000)
}
fn iec(n: u64) -> String {
    human(n, IECU, 1024)
}

/// 速率：bytes/s 以 SI 單位（如 "3.1 MB/s"）；<1000 → "N B/s"。
fn rate(bytes: u64, secs: f64) -> String {
    if secs <= 0.0 {
        return format!("{bytes} B/s");
    }
    let r = bytes as f64 / secs;
    // 對 r 做 SI human（不含空格版搭配 dd 格式）
    let mut exp = 0usize;
    let mut scale = 1f64;
    while exp + 1 < SIU.len() && r >= scale * 1000.0 {
        scale *= 1000.0;
        exp += 1;
    }
    if exp == 0 {
        return format!("{:.1} B/s", r);
    }
    let v = r / scale;
    format!("{:.1} {}", v, SIU[exp])
}
