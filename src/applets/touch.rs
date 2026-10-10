// touch：更新時間戳。措辭對齊參考 uutils coreutils 0.10.0。
//
// - 錯誤："touch: cannot touch 'X': <reason（無 os error 後綴）>"；
//   無 operand："touch: missing file operand" rc1（參考附 "Try ..." 行，不追隨）；
//   日期無法解析："touch: Unable to parse date: {d}" rc1
// - 日期格式：@epoch、"YYYY-MM-DD"、"YYYY-MM-DD[ T]HH:MM[:SS]"（**本地時區**）；
//   自然語言（"next friday"）**不實作**（assumptions）
// - -t [[CC]YY]MMDDhhmm[.ss]：**UTC**（參考 uutils 行徑；assumptions）
// - 不存在的檔案會建立（-c 除外）

use std::fs;

extern "C" {
    fn utimensat(dirfd: i32, path: *const i8, times: *const [i64; 4], flags: i32) -> i32;
}

const UTIME_OMIT: i64 = 0x3fff_fff6 + 8; // 0x3ffffffe
const UTIME_NOW: i64 = 0x3fff_fff1 + 14; // 0x3fffffff

pub fn run(args: &[String], _stdin: crate::In, stdout: crate::Out, stderr: crate::Out) -> i32 {
    let mut atime_only = false;
    let mut mtime_only = false;
    let mut no_create = false;
    let mut no_follow = false;
    let mut date: Option<i64> = None;
    let mut ref_file: Option<String> = None;
    let mut files: Vec<String> = Vec::new();
    let mut i = 0usize;
    let mut only_operands = false;
    while i < args.len() {
        let a = args[i].as_str();
        if only_operands {
            files.push(a.to_string());
            i += 1;
            continue;
        }
        if a == "--" {
            only_operands = true;
            i += 1;
            continue;
        }
        if let Some(long) = a.strip_prefix("--") {
            let (name, val) = match long.split_once('=') {
                Some((n, v)) => (n, Some(v.to_string())),
                None => (long, None),
            };
            let getval = |i: &mut usize| -> Option<String> {
                if let Some(v) = val.clone() {
                    return Some(v);
                }
                *i += 1;
                args.get(*i).cloned()
            };
            match name {
                "date" => match getval(&mut i) {
                    Some(v) => match parse_date(&v) {
                        Some(t) => date = Some(t),
                        None => {
                            writeln!(stderr, "touch: Unable to parse date: {}", v).ok();
                            return 1;
                        }
                    },
                    None => {
                        writeln!(stderr, "touch: option requires an argument -- 'date'").ok();
                        return 2;
                    }
                },
                "reference" => match getval(&mut i) {
                    Some(v) => ref_file = Some(v),
                    None => {
                        writeln!(stderr, "touch: option requires an argument -- 'reference'").ok();
                        return 2;
                    }
                },
                "no-create" => no_create = true,
                "no-dereference" => no_follow = true,
                "help" => {
                    writeln!(stdout, "Usage: touch [OPTION]... FILE...\nUpdate file access and modification times").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "touch (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => {
                    writeln!(stderr, "touch: unrecognized option '{}'", a).ok();
                    return 2;
                }
            }
            i += 1;
            continue;
        }
        if a.len() > 1 && a.starts_with('-') {
            let chars: Vec<char> = a[1..].chars().collect();
            let mut k = 0usize;
            while k < chars.len() {
                match chars[k] {
                    'a' => atime_only = true,
                    'm' => mtime_only = true,
                    'c' => no_create = true,
                    'h' => no_follow = true,
                    'd' | 't' | 'r' => {
                        let opt = chars[k];
                        let rest: String = chars[k + 1..].iter().collect();
                        let val = if !rest.is_empty() {
                            Some(rest)
                        } else {
                            i += 1;
                            args.get(i).cloned()
                        };
                        let Some(v) = val else {
                            writeln!(stderr, "touch: option requires an argument -- '{}'", opt).ok();
                            return 2;
                        };
                        match opt {
                            'd' => match parse_date(&v) {
                                Some(t) => date = Some(t),
                                None => {
                                    writeln!(stderr, "touch: Unable to parse date: {}", v).ok();
                                    return 1;
                                }
                            },
                            't' => match parse_stamp(&v) {
                                Some(t) => date = Some(t),
                                None => {
                                    writeln!(stderr, "touch: invalid date format '{}'", v).ok();
                                    return 1;
                                }
                            },
                            _ => ref_file = Some(v),
                        }
                        k = chars.len();
                        continue;
                    }
                    c => {
                        writeln!(stderr, "touch: invalid option -- '{}'", c).ok();
                        return 2;
                    }
                }
                k += 1;
            }
            i += 1;
            continue;
        }
        files.push(a.to_string());
        i += 1;
    }
    if files.is_empty() {
        writeln!(stderr, "touch: missing file operand").ok();
        return 1;
    }

    let ref_times: Option<(i64, i64)> = match ref_file {
        Some(rf) => match stat_times(rf.as_str()) {
            Ok(t) => Some(t),
            Err(e) => {
                writeln!(stderr, "touch: failed to get attributes of '{}': {}", rf, crate::pathname::io_reason_nosuffix(&e)).ok();
                return 1;
            }
        },
        None => None,
    };
    let mut rc = 0;
    for f in &files {
        if let Err(e) = touch_one(f, atime_only, mtime_only, no_create, no_follow, date, ref_times) {
            writeln!(stderr, "touch: cannot touch '{}': {}", f, crate::pathname::io_reason_nosuffix(&e)).ok();
            rc = 1;
        }
    }
    rc
}

fn touch_one(
    path: &str,
    atime_only: bool,
    mtime_only: bool,
    no_create: bool,
    no_follow: bool,
    date: Option<i64>,
    ref_times: Option<(i64, i64)>,
) -> std::io::Result<()> {
    use std::ffi::CString;
    let exists = if no_follow {
        fs::symlink_metadata(path).is_ok()
    } else {
        fs::metadata(path).is_ok()
    };
    if !exists {
        if no_create {
            return Ok(());
        }
        fs::OpenOptions::new().create(true).append(true).open(path).map(|_| ())?;
    }
    // 選取時間來源
    let src: Option<(i64, i64)> = match (date, ref_times) {
        (Some(t), _) => Some((t, t)),
        (None, Some(tm)) => Some(tm),
        (None, None) => None, // → UTIME_NOW
    };
    // struct timespec[2]：[atime_sec, atime_nsec, mtime_sec, mtime_nsec]
    let mut ts = [0i64, UTIME_OMIT, 0i64, UTIME_OMIT];
    let set_a = !mtime_only;
    let set_m = !atime_only;
    match src {
        Some((a, m)) => {
            if set_a {
                ts[0] = a;
                ts[1] = 0;
            }
            if set_m {
                ts[2] = m;
                ts[3] = 0;
            }
        }
        None => {
            if set_a {
                ts[1] = UTIME_NOW;
            }
            if set_m {
                ts[3] = UTIME_NOW;
            }
        }
    }
    let c = CString::new(path).map_err(|_| std::io::Error::from_raw_os_error(22))?;
    let flags = if no_follow { 0x100 } else { 0 };
    if unsafe { utimensat(-100, c.as_ptr(), &ts as *const [i64; 4], flags) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok(())
}

/// stat(2) 取 atime/mtime（struct stat x86_64：st_atime 在偏移 72、st_mtime 88（x86_64：nlink 在 mode 之前））。
fn stat_times(path: &str) -> std::io::Result<(i64, i64)> {
    use std::ffi::CString;
    let c = CString::new(path).map_err(|_| std::io::Error::from_raw_os_error(22))?;
    let mut st = crate::sys::StatC {
        unused: [0; 3],
        st_dev: 0, st_ino: 0, st_mode: 0, st_nlink: 0, st_uid: 0, st_gid: 0,
        st_pad0: 0, st_rdev: 0, st_size: 0, st_blksize: 0, st_blocks: 0,
        st_atime: 0, st_atime_nsec: 0, st_mtime: 0, st_mtime_nsec: 0, st_ctime: 0, st_ctime_nsec: 0,
    };
    if unsafe { crate::sys::stat(c.as_ptr(), &mut st) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    Ok((st.st_atime, st.st_mtime))
}

/// @epoch / ISO 本地時間。
pub fn parse_date(s: &str) -> Option<i64> {
    let s = s.trim();
    if let Some(e) = s.strip_prefix('@') {
        return e.parse::<i64>().ok();
    }
    let (ds, ts) = match s.find([' ', 'T']) {
        Some(i) => (&s[..i], Some(&s[i + 1..])),
        None => (s, None),
    };
    let dp: Vec<&str> = ds.split('-').collect();
    if dp.len() != 3 {
        return None;
    }
    let y: i64 = dp[0].parse().ok()?;
    let mo: i64 = dp[1].parse().ok()?;
    let d: i64 = dp[2].parse().ok()?;
    let (h, mi, sec) = match ts {
        None => (0, 0, 0),
        Some(t) => {
            let t = t.trim_end();
            let tp: Vec<&str> = t.split(':').collect();
            match tp.len() {
                1 => (tp[0].parse().ok()?, 0, 0),
                2 => (tp[0].parse().ok()?, tp[1].parse().ok()?, 0),
                3 => (tp[0].parse().ok()?, tp[1].parse().ok()?, tp[2].parse::<f64>().ok()? as i64),
                _ => return None,
            }
        }
    };
    if !(1970..=9999).contains(&y) || !(1..=12).contains(&mo) || !(1..=31).contains(&d) {
        return None;
    }
    if h > 23 || mi > 59 || sec > 60 {
        return None;
    }
    let naive = days_from_civil(y, mo, d) * 86400 + h * 3600 + mi * 60 + sec;
    // 本地時區：先以 naive 估 gmtoff，再校正
    let off1 = gmtoff_of(naive);
    let guess = naive - off1;
    let off2 = gmtoff_of(guess);
    Some(naive - off2)
}

/// -t [[CC]YY]MMDDhhmm[.ss]：UTC（參考 uutils 行徑）。
pub fn parse_stamp(s: &str) -> Option<i64> {
    let (main, ss) = match s.split_once('.') {
        Some((m, sec)) => (m, sec.parse::<i64>().ok()?),
        None => (s, 0),
    };
    if main.is_empty() || !main.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    // GNU: 8=MMDDhhmm(本年) 10=YYMMDDhhmm(YY>=69→19xx) 12=CCYYMMDDhhmm；
    // 參考（uutils）把 8 位也當無年份？實測 12/10 皆可。
    let (y, mm, dd, hh, mi) = match main.len() {
        8 => {
            let now = crate::sys::local_tm(now_epoch()).map(|t| t.tm_year as i64 + 1900).unwrap_or(1970);
            (now, &main[0..2], &main[2..4], &main[4..6], &main[6..8])
        }
        10 => {
            let yy: i64 = main[0..2].parse().ok()?;
            let y = if yy >= 69 { 1900 + yy } else { 2000 + yy };
            (y, &main[2..4], &main[4..6], &main[6..8], &main[8..10])
        }
        12 => (
            main[0..4].parse::<i64>().ok()?,
            &main[4..6],
            &main[6..8],
            &main[8..10],
            &main[10..12],
        ),
        _ => return None,
    };
    let (y2, mm2, dd2, hh2, mi2) = (y, mm, dd, hh, mi);
    let mo: i64 = mm2.parse().ok()?;
    let d: i64 = dd2.parse().ok()?;
    let h: i64 = hh2.parse().ok()?;
    let mi: i64 = mi2.parse().ok()?;
    if !(1..=12).contains(&mo) || !(1..=31).contains(&d) || h > 23 || mi > 59 || ss > 60 {
        return None;
    }
    Some(days_from_civil(y2, mo, d) * 86400 + h * 3600 + mi * 60 + ss)
}

fn now_epoch() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn gmtoff_of(epoch: i64) -> i64 {
    crate::sys::local_tm(epoch).map(|t| t.tm_gmtoff).unwrap_or(0)
}

/// Howard Hinnant days_from_civil。
fn days_from_civil(y: i64, m: i64, d: i64) -> i64 {
    let y = if m <= 2 { y - 1 } else { y };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let mp = if m > 2 { m - 3 } else { m + 9 };
    let doy = (153 * mp + 2) / 5 + d - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146097 + doe - 719468
}
