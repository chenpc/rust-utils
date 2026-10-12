//! 共用「執行外部命令」輔助（nohup / nice / timeout / chroot / env 都用）。
//!
//! 這些 applet 在 GNU/uutils 裡是 exec(3) 取代行程；本專案的測試是 in-process
//! （`src/testutil.rs` 直接呼叫 dispatch），exec 會殺掉 test binary，因此一律改用
//! `std::process::Command` spawn + wait（差異記錄在 docs/assumptions.md）。

use std::path::PathBuf;

/// PATH 查詢：找不到 → None；找到但不可執行 → 該路徑。
pub fn resolve_program(cmd: &str) -> Option<PathBuf> {
    if cmd.contains('/') {
        let p = PathBuf::from(cmd);
        return p.exists().then_some(p);
    }
    let path = std::env::var("PATH").unwrap_or_default();
    let dirs: Vec<String> = if path.is_empty() {
        vec!["/bin".into(), "/usr/bin".into()]
    } else {
        path.split(':').map(|s| s.to_string()).collect()
    };
    for d in dirs {
        let cand = PathBuf::from(d).join(cmd);
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

pub fn is_executable(path: &PathBuf) -> bool {
    match std::fs::metadata(path) {
        Ok(md) => md.is_file() && (md.st_mode() & 0o111 != 0),
        Err(_) => false,
    }
}

use std::os::linux::fs::MetadataExt;
use std::os::unix::process::ExitStatusExt;

/// 產生 exit code：被訊號殺 → 128+sig。
pub fn status_code(status: std::process::ExitStatus) -> i32 {
    match status.code() {
        Some(c) => c,
        None => 128 + status.signal().unwrap_or(0),
    }
}

pub fn errno_reason(err: &std::io::Error) -> String {
    err.raw_os_error()
        .map(|c| std::io::Error::from_raw_os_error(c).to_string())
        .unwrap_or_else(|| err.to_string())
}

// ---- libc FFI（這些 applet 需要；src/sys.rs 已有 uname/time/tty 等其他符號）----

pub const PRIO_PROCESS: i32 = 0;
pub const SIG_DFL: usize = 0;
pub const SIG_IGN: usize = 1;
pub const SIG_ERR: usize = usize::MAX;

extern "C" {
    #[link_name = "kill"]
    pub fn c_kill(pid: i32, sig: i32) -> i32;
    #[link_name = "getpriority"]
    pub fn c_getpriority(which: i32, who: u32) -> i32;
    #[link_name = "setpriority"]
    pub fn c_setpriority(which: i32, who: u32, prio: i32) -> i32;
    #[link_name = "isatty"]
    pub fn c_isatty(fd: i32) -> i32;
    #[link_name = "signal"]
    pub fn c_signal(sig: i32, disp: usize) -> usize;
    #[link_name = "chroot"]
    pub fn c_chroot(path: *const i8) -> i32;
    #[link_name = "setuid"]
    pub fn c_setuid(uid: u32) -> i32;
    #[link_name = "setgid"]
    pub fn c_setgid(gid: u32) -> i32;
    #[link_name = "setgroups"]
    pub fn c_setgroups(n: usize, list: *const u32) -> i32;
    #[link_name = "initgroups"]
    pub fn c_initgroups(user: *const i8, gid: u32) -> i32;
    #[link_name = "chdir"]
    pub fn c_chdir(path: *const i8) -> i32;
    #[link_name = "write"]
    pub fn c_write(fd: i32, buf: *const u8, count: usize) -> isize;
}

pub fn is_tty(fd: i32) -> bool {
    unsafe { c_isatty(fd) == 1 }
}

/// 目前行程的 niceness（getpriority 失敗 → 0）。
pub fn current_niceness() -> i32 {
    unsafe {
        let r = c_getpriority(PRIO_PROCESS, 0);
        if r == -1 {
            0
        } else {
            r
        }
    }
}

/// 取得 /etc/passwd、/etc/group 的名字 → id（給 chroot --userspec / --groups）。
pub fn lookup_user(name: &str) -> Option<u32> {
    if let Ok(v) = name.parse::<u32>() {
        return Some(v);
    }
    passwd_entries().into_iter().find(|(n, _)| n == name).map(|(_, id)| id)
}

pub fn lookup_group(name: &str) -> Option<u32> {
    if let Ok(v) = name.parse::<u32>() {
        return Some(v);
    }
    group_entries().into_iter().find(|(n, _)| n == name).map(|(_, id)| id)
}

fn parse_id_file(path: &str, id_field: usize) -> Vec<(String, u32)> {
    let mut out = Vec::new();
    if let Ok(text) = std::fs::read_to_string(path) {
        for line in text.lines() {
            if line.starts_with('#') {
                continue;
            }
            let fields: Vec<&str> = line.split(':').collect();
            if fields.len() <= id_field {
                continue;
            }
            if let Ok(id) = fields[id_field].parse::<u32>() {
                out.push((fields[0].to_string(), id));
            }
        }
    }
    out
}

fn passwd_entries() -> Vec<(String, u32)> {
    parse_id_file("/etc/passwd", 2)
}

fn group_entries() -> Vec<(String, u32)> {
    parse_id_file("/etc/group", 2)
}

pub const SIG_TABLE: &[(i32, &str)] = &[
    (1, "HUP"),
    (2, "INT"),
    (3, "QUIT"),
    (4, "ILL"),
    (5, "TRAP"),
    (6, "ABRT"),
    (7, "BUS"),
    (8, "FPE"),
    (9, "KILL"),
    (10, "USR1"),
    (11, "SEGV"),
    (12, "USR2"),
    (13, "PIPE"),
    (14, "ALRM"),
    (15, "TERM"),
    (16, "STKFLT"),
    (17, "CHLD"),
    (18, "CONT"),
    (19, "STOP"),
    (20, "TSTP"),
    (21, "TTIN"),
    (22, "TTOU"),
    (23, "URG"),
    (24, "XCPU"),
    (25, "XFSZ"),
    (26, "VTALRM"),
    (27, "PROF"),
    (28, "WINCH"),
    (29, "PWR"),
    (30, "SYS"),
    (34, "RTMIN"),
    (64, "RTMAX"),
];

/// 訊號名稱或數字 → 編號（不接受 `sig9` 這類前綴形式，與 env 一致）。
pub fn signal_number(spec: &str) -> Option<i32> {
    if let Ok(n) = spec.parse::<i32>() {
        if (1..=64).contains(&n) {
            return Some(n);
        }
        return None;
    }
    let name = spec.to_ascii_uppercase();
    let name = name.strip_prefix("SIG").unwrap_or(&name);
    SIG_TABLE.iter().find(|(_, n)| *n == name).map(|(s, _)| *s)
}

/// 編號 → 名稱（表中沒有時用 `SIG<n>`）。
pub fn signal_name(sig: i32) -> String {
    SIG_TABLE
        .iter()
        .find(|(s, _)| *s == sig)
        .map(|(_, n)| n.to_string())
        .unwrap_or_else(|| format!("SIG{}", sig))
}

/// `NUMBER[SUFFIX]` → 秒（s 預設、m/h/d；負數或非有限 → None）。
pub fn parse_duration(s: &str) -> Option<f64> {
    let (body, mult) = match s.chars().last() {
        Some('s') => (&s[..s.len() - 1], 1.0),
        Some('m') => (&s[..s.len() - 1], 60.0),
        Some('h') => (&s[..s.len() - 1], 3600.0),
        Some('d') => (&s[..s.len() - 1], 86400.0),
        _ => (s, 1.0),
    };
    if body.is_empty() {
        return None;
    }
    let v = body.parse::<f64>().ok()?;
    if !v.is_finite() || v < 0.0 {
        return None;
    }
    Some(v * mult)
}
