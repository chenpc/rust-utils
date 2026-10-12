//! /etc/passwd 與 /etc/group 的輕量解析。
//!
//! 不用 NSS（`getpwnam`/`getgrgid`）：本專案的 binary 以 `-C target-feature=+crt-static`
//! 靜態連結，靜態 glibc 的 NSS 模組載入（dlopen `libnss_files.so.2`）不可靠；直接解析
//! 檔案行為可預測。實測佐證：在 bwrap 沙箱內放自訂 `/etc/passwd`＋`/etc/group`，本機參考
//! （uutils coreutils 0.10.0）即依該兩個檔案解析出名稱（見 docs/assumptions.md）。

use std::fs;

/// /etc/passwd 的一筆記錄（欄位依 POSIX：name:passwd:uid:gid:gecos:dir:shell）。
#[derive(Debug, Clone)]
pub struct Passwd {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub gecos: String,
    pub dir: String,
    pub shell: String,
}

fn parse_passwd_line(line: &str) -> Option<Passwd> {
    let mut it = line.split(':');
    let name = it.next()?.to_string();
    let _pw = it.next()?;
    let uid = it.next()?.parse::<u32>().ok()?;
    let gid = it.next()?.parse::<u32>().ok()?;
    let gecos = it.next().unwrap_or("").to_string();
    let dir = it.next().unwrap_or("").to_string();
    let shell = it.next().unwrap_or("").to_string();
    if name.is_empty() {
        return None;
    }
    Some(Passwd {
        name,
        uid,
        gid,
        gecos,
        dir,
        shell,
    })
}

fn passwd_entries() -> Vec<Passwd> {
    let text = match fs::read_to_string("/etc/passwd") {
        Ok(t) => t,
        Err(_) => return Vec::new(),
    };
    let mut out = Vec::new();
    for line in text.lines() {
        if let Some(e) = parse_passwd_line(line) {
            out.push(e);
        }
    }
    out
}

/// 依 uid 找 passwd 記錄；找不到回 None。
pub fn passwd_by_uid(uid: u32) -> Option<Passwd> {
    passwd_entries().into_iter().find(|e| e.uid == uid)
}

/// 依名稱找 passwd 記錄。
pub fn passwd_by_name(name: &str) -> Option<Passwd> {
    passwd_entries()
        .into_iter()
        .find(|e| e.name == name)
}

/// operand 解析：純數字 → 依 uid，否則依名稱（參考行為：`id 1000`、`id root` 皆可）。
pub fn passwd_by_operand(operand: &str) -> Option<Passwd> {
    if !operand.is_empty() && operand.chars().all(|c| c.is_ascii_digit()) {
        match operand.parse::<u32>() {
            Ok(uid) => return passwd_by_uid(uid),
            Err(_) => return None,
        }
    }
    passwd_by_name(operand)
}

/// gid → 群組名稱（讀 /etc/group）；找不到回 None。
pub fn group_name(gid: u32) -> Option<String> {
    let text = match fs::read_to_string("/etc/group") {
        Ok(t) => t,
        Err(_) => return None,
    };
    for line in text.lines() {
        let mut it = line.split(':');
        let name = match it.next() {
            Some(n) if !n.is_empty() => n,
            _ => continue,
        };
        let _pw = it.next();
        let id = match it.next().and_then(|s| s.parse::<u32>().ok()) {
            Some(id) => id,
            None => continue,
        };
        if id == gid {
            return Some(name.to_string());
        }
    }
    None
}

/// 名稱對應的群組清單：gid 先，然後依 /etc/group 檔案順序列出成員含該名稱的群組
/// （已出現的 gid 不重複）。實測佐證：`id t2` → `groups=200(grpB),100(grpA)`。
pub fn groups_for(name: &str, gid: u32) -> Vec<u32> {
    let mut out = vec![gid];
    let text = match fs::read_to_string("/etc/group") {
        Ok(t) => t,
        Err(_) => return out,
    };
    for line in text.lines() {
        let mut it = line.split(':');
        let gname = match it.next() {
            Some(n) if !n.is_empty() => n,
            _ => continue,
        };
        let _pw = it.next();
        let id = match it.next().and_then(|s| s.parse::<u32>().ok()) {
            Some(id) => id,
            None => continue,
        };
        let members = it.next().unwrap_or("");
        if id == gid {
            continue;
        }
        if members.split(',').any(|m| m == name) {
            out.push(id);
        }
        let _ = gname;
    }
    out
}
