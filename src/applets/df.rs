//! df(1) — Report file system space usage.
//!
//! 依據 man page（docs/man/df.1.gz，GNU 9.7）並與本機參考實作（uutils 0.10.0）逐 case 對 diff：
//! - 預設列：Filesystem / 1K-blocks / Used / Available / Use% / Mounted on
//! - `-P`：POSIX 表頭（1024-blocks / Capacity）
//! - `-T`：加 Type 欄；`-i`：inode 欄位；`--total`：加 total 列（Mounted on 為 `-`）
//! - `-a` 含偽裝檔案系統；`-t`/`-x` 型別過濾；`-l` 略過網路型別
//! - `-B SIZE`/`-k`/環境變數 DF_BLOCK_SIZE→BLOCK_SIZE→BLOCKSIZE；POSIXLY_CORRECT → 512
//! - `-h`/`-H`：1024/1000 為底的人類可讀單位
//! - Used = blocks - bfree；Use% = ceil(used*100/(used+avail))；`df FILE` 找最長字首掛載點

use std::io::Write;

use crate::sys;
use crate::{In, Out};

const VERSION: &str = "df (uutils coreutils) 0.10.0";
const USAGE: &str = "Usage: df [OPTION]... [FILE]...";

/// 預設略過的偽裝檔案系統（-a 收錄）。
const PSEUDO: &[&str] = &[
    "proc",
    "procfs",
    "sysfs",
    "cgroup",
    "cgroup2",
    "devpts",
    "mqueue",
    "bpf",
    "configfs",
    "debugfs",
    "tracefs",
    "securityfs",
    "pstore",
    "binfmt_misc",
    "autofs",
    "fusectl",
    "ramfs",
    "hugetlbfs",
    "rpc_pipefs",
    "selinuxfs",
    "nsfs",
    "fuse.portal",
    "fuse.gvfsd-fuse",
];

/// 網路檔案系統型別（-l 略過）。
const NETWORK: &[&str] = &["nfs", "nfs4", "cifs", "smbfs", "sshfs", "9p", "glusterfs", "ceph", "afs", "fuse.sshfs"];

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut all = false;
    let mut local = false;
    let mut inodes = false;
    let mut human = 0u8; // 0 = none, 1 = 1024, 2 = 1000
    let mut portability = false;
    let mut print_type = false;
    let mut total = false;
    let mut do_sync = false;
    let mut types: Vec<String> = Vec::new();
    let mut ex_types: Vec<String> = Vec::new();
    let mut block_size: u64 = 0; // 0 → 依環境/預設
    let mut operands: Vec<String> = Vec::new();
    let mut output_fields: Option<Vec<String>> = None;
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
                Some(k) => (&a[2..2 + k], Some(a[2 + k + 1..].to_string())),
                None => (&a[2..], None),
            };
            match name {
                "all" => all = true,
                "local" => local = true,
                "inodes" => inodes = true,
                "human-readable" => human = 1,
                "si" => human = 2,
                "portability" => portability = true,
                "print-type" => print_type = true,
                "total" => total = true,
                "sync" => do_sync = true,
                "no-sync" => do_sync = false,
                "block-size" | "output" => {
                    if name == "output" {
                        output_fields = Some(match inline {
                            Some(v) => v.split(',').map(|s| s.to_string()).collect(),
                            None => vec![
                                "source".into(),
                                "size".into(),
                                "used".into(),
                                "avail".into(),
                                "pcent".into(),
                                "target".into(),
                            ],
                        });
                        continue;
                    }
                    let v = match inline {
                        Some(v) => v,
                        None => {
                            if i < args.len() {
                                let v = args[i].clone();
                                i += 1;
                                v
                            } else {
                                writeln!(stderr, "df: option requires an argument -- 'B'").ok();
                                return 2;
                            }
                        }
                    };
                    match parse_size(&v) {
                        Some(n) => {
                            block_size = n;
                        }
                        None => {
                            writeln!(stderr, "df: invalid block size '{v}'").ok();
                            return 2;
                        }
                    }
                }
                "type" | "exclude-type" => {
                    let v = match inline {
                        Some(v) => v,
                        None => {
                            if i < args.len() {
                                let v = args[i].clone();
                                i += 1;
                                v
                            } else {
                                writeln!(stderr, "df: option requires an argument -- 't'").ok();
                                return 2;
                            }
                        }
                    };
                    if name == "type" {
                        types.push(v);
                    } else {
                        ex_types.push(v);
                    }
                }
                "help" => {
                    writeln!(stdout, "{USAGE}").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "{VERSION}").ok();
                    return 0;
                }
                name => {
                    writeln!(stderr, "df: unrecognized option '--{name}'").ok();
                    return 2;
                }
            }
            continue;
        }
        // 短選項組：-B 可附值（-BM）或取下一參數（-B M）；-hHikPTa 為旗標
        let chars: Vec<char> = a[1..].chars().collect();
        let mut k = 0usize;
        while k < chars.len() {
            match chars[k] {
                'a' => all = true,
                'l' => local = true,
                'i' => inodes = true,
                'h' => human = 1,
                'H' => human = 2,
                'P' => portability = true,
                'T' => print_type = true,
                'k' => block_size = 1024,
                'B' => {
                    let rest: String = chars[k + 1..].iter().collect();
                    let v = if !rest.is_empty() {
                        rest
                    } else if i < args.len() {
                        let v = args[i].clone();
                        i += 1;
                        v
                    } else {
                        writeln!(stderr, "df: option requires an argument -- 'B'").ok();
                        return 2;
                    };
                    match parse_size(&v) {
                        Some(n) => {
                            block_size = n;
                        }
                        None => {
                            writeln!(stderr, "df: invalid block size '{v}'").ok();
                            return 2;
                        }
                    }
                    k = chars.len();
                    continue;
                }
                't' | 'x' => {
                    let rest: String = chars[k + 1..].iter().collect();
                    let v = if !rest.is_empty() {
                        rest
                    } else if i < args.len() {
                        let v = args[i].clone();
                        i += 1;
                        v
                    } else {
                        writeln!(stderr, "df: option requires an argument -- '{}'", chars[k]).ok();
                        return 2;
                    };
                    if chars[k] == 't' {
                        types.push(v);
                    } else {
                        ex_types.push(v);
                    }
                    k = chars.len();
                    continue;
                }
                'v' => {
                    // 參考行為：-v 不是 df 的選項（ref 由 clap 拒絕，rc=1）；我方用 GNU 風格 rc=2
                    writeln!(stderr, "df: invalid option -- 'v'").ok();
                    return 2;
                }
                c => {
                    writeln!(stderr, "df: invalid option -- '{c}'").ok();
                    return 2;
                }
            }
            k += 1;
        }
    }

    // 區塊單位：-B > 環境 DF_BLOCK_SIZE > BLOCK_SIZE > BLOCKSIZE > 1024（POSIXLY_CORRECT→512）
    if block_size == 0 {
        // 參考 uutils 0.10.0：非 -P 時依序環境 DF_BLOCK_SIZE > BLOCK_SIZE > BLOCKSIZE
        //（無效值忽略）；-P 忽略環境；最後 POSIXLY_CORRECT→512，否則 1024
        if !portability {
            for var in ["DF_BLOCK_SIZE", "BLOCK_SIZE", "BLOCKSIZE"] {
                if let Ok(v) = std::env::var(var) {
                    if let Some(n) = parse_size(&v) {
                        block_size = n;
                        break;
                    }
                }
            }
        }
    }
    if block_size == 0 {
        block_size = if std::env::var_os("POSIXLY_CORRECT").is_some() {
            512
        } else {
            1024
        };
    }

    // 表頭單位標籤：由實際區塊大小縮寫（512→"512B"、4096→"4K"、1536→"1.6kB"）
    let unit_label = label_from_size(block_size);

    if do_sync {
        sys::sync_all();
    }

    // 收集掛載
    let mounts = match read_mounts() {
        Ok(m) => m,
        Err(e) => {
            writeln!(stderr, "df: cannot read mount table: {e}").ok();
            return 1;
        }
    };

    let mut entries: Vec<Entry> = Vec::new();
    let mut status = 0;
    if operands.is_empty() {
        let mut seen: Vec<u64> = Vec::new();
        for m in &mounts {
            if m.fstype == "autofs" {
                // 參考行為：autofs 自動掛載行（如 systemd-1 binfmt_misc）一律不列，-a 也不列
                continue;
            }
            if !all && PSEUDO.contains(&m.fstype.as_str()) {
                continue;
            }
            if local && NETWORK.contains(&m.fstype.as_str()) {
                continue;
            }
            if !types.is_empty() && !types.contains(&m.fstype) {
                continue;
            }
            if ex_types.contains(&m.fstype) {
                continue;
            }
            match sys::statvfs(&m.mount) {
                Ok(st) => {
                    // 去重鍵：GNU 用掛載點 st_dev（9p 兩掛載 f_fsid 相同但 st_dev 不同）
                    let dev = sys::stat_dev(&m.mount).unwrap_or(0);
                    if dev != 0 && seen.contains(&dev) {
                        continue;
                    }
                    if dev != 0 {
                        seen.push(dev);
                    }
                    entries.push(Entry {
                        source: m.source.clone(),
                        fstype: m.fstype.clone(),
                        target: m.mount.clone(),
                        st,
                    });
                }
                Err(_) => {
                    // 參考行為：列舉時 statvfs 失敗 → 靜默跳過，不影響 exit code
                }
            }
        }
    } else {
        let mut seen_dev: Vec<u64> = Vec::new();
        for f in &operands {
            let st = match sys::statvfs(f) {
                Ok(st) => st,
                Err(e) => {
                    writeln!(stderr, "df: {f}: {}", io_reason(&e)).ok();
                    status = 1;
                    continue;
                }
            };
            // 以 canonical 路徑找最長字首掛載點
            let abs = canon_for_lookup(f);
            let m = mounts
                .iter()
                .filter(|m| abs == m.mount || abs.starts_with(&format!("{}/", m.mount)) || m.mount == "/")
                .max_by_key(|m| m.mount.len());
            let (source, fstype, target) = match m {
                Some(m) => (m.source.clone(), m.fstype.clone(), m.mount.clone()),
                None => (abs.clone(), String::new(), abs.clone()),
            };
            let dev = sys::stat_dev(f).unwrap_or(st.f_fsid);
            if !seen_dev.contains(&dev) {
                seen_dev.push(dev);
            } else if !all {
                continue;
            }
            entries.push(Entry { source, fstype, target, st });
        }
    }

    print_table(&entries, block_size, &unit_label, human, inodes, portability, print_type, total, output_fields, &mounts.iter().map(|m| m.source.len()).collect::<Vec<usize>>(), stdout);
    if status != 0 {
        return 1;
    }
    if operands.is_empty() && entries.is_empty() {
        // 參考行為：過濾後沒有任何列 → 錯誤訊息 rc=1
        writeln!(stderr, "df: no file systems processed").ok();
        return 1;
    }
    0
}

struct Mount {
    source: String,
    mount: String,
    fstype: String,
}

fn read_mounts() -> std::io::Result<Vec<Mount>> {
    let text = std::fs::read_to_string("/proc/mounts")?;
    let mut out = Vec::new();
    for line in text.lines() {
        let mut it = line.split_whitespace();
        let (Some(src), Some(mnt), Some(ft)) = (it.next(), it.next(), it.next()) else {
            continue;
        };
        // /proc/mounts 中空格以 \040 轉義
        let unescape = |s: &str| s.replace("\\040", " ").replace("\\011", "\t").replace("\\012", "\n").replace("\\134", "\\");
        out.push(Mount {
            source: unescape(src),
            mount: unescape(mnt),
            fstype: unescape(ft),
        });
    }
    Ok(out)
}

fn io_reason(e: &std::io::Error) -> String {
    let s = e.to_string();
    match s.rfind(" (os error ") {
        Some(k) => s[..k].to_string(),
        None => s,
    }
}

fn canon_for_lookup(f: &str) -> String {
    if f.is_empty() {
        return String::new();
    }
    match std::fs::canonicalize(f) {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(_) => {
            // 不存在時退而求其次：拼 cwd 後詞法化
            let joined = if f.starts_with('/') {
                f.to_string()
            } else {
                let cwd = std::env::current_dir().map(|c| c.to_string_lossy().to_string()).unwrap_or_default();
                format!("{cwd}/{f}")
            };
            let mut comps: Vec<&str> = Vec::new();
            for c in joined.split('/') {
                match c {
                    "" | "." => {}
                    ".." => {
                        comps.pop();
                    }
                    _ => comps.push(c),
                }
            }
            format!("/{}", comps.join("/"))
        }
    }
}

struct Entry {
    source: String,
    fstype: String,
    target: String,
    st: sys::Statvfs,
}

impl Entry {
    /// 換算成區塊單位的 (size, used, avail)。
    fn blocks(&self, unit: u64) -> (u64, u64, u64) {
        let frsize = if self.st.f_frsize != 0 { self.st.f_frsize } else { self.st.f_bsize };
        let conv = |n: u64| -> u64 {
            let bytes = n as u128 * frsize as u128;
            ((bytes + unit as u128 - 1) / unit as u128) as u64 // 向上取整
        };
        (conv(self.st.f_blocks), conv(self.st.f_blocks - self.st.f_bfree), conv(self.st.f_bavail))
    }
    fn inos(&self) -> (u64, u64, u64) {
        (self.st.f_files, self.st.f_files - self.st.f_ffree, self.st.f_favail)
    }
}

fn pct(used: u64, avail: u64) -> u64 {
    let tot = used + avail;
    if tot == 0 {
        0
    } else {
        (used * 100 + tot - 1) / tot // ceil
    }
}

const UNITS1024: &[&str] = &["", "K", "M", "G", "T", "P", "E", "Z", "Y"];
const UNITS1000: &[&str] = &["k", "M", "G", "T", "P", "E", "Z", "Y"];

/// GNU/uutils 風格人類可讀（由 host diff 推導）：
/// - 選最大 unit 使 value < base
/// - value < 10：固定一個小數位，小數**向上取整**；湊到 10.0 → 進位到下一 unit
/// - value >= 10：整數（四捨五入）；>= base → 進位到下一 unit
/// - exp == 0（bytes < base）→ 直接印位元組數（df -h 的 unit 最小為 K）
fn humanize(bytes: u128, si: bool) -> String {
    if bytes == 0 {
        return "0".to_string();
    }
    let base: u128 = if si { 1000 } else { 1024 };
    let units: &[&str] = if si { UNITS1000 } else { &UNITS1024[1..] };
    // 找 exp：bytes >= base^(exp+1) → 進位；units[exp] 對應 base^(exp+1)
    let mut exp = 0usize;
    let mut scale = base;
    while exp + 1 < units.len() && bytes >= scale * base {
        scale *= base;
        exp += 1;
    }
    if exp == 0 && bytes < base {
        return format!("{bytes}");
    }
    let suffix = units[exp];
    // 參考行為（host diff 推導）：所有顯示精度一律**向上取整**（ceil）：
    // 12.54G→"13G"、31.36G→"32G"、1.832T→"1.9T"、1.625M→"1.7M"、exactly 1.0M→"1.0M"
    if bytes < scale * 10 {
        // 一個小數位
        let mut sc = scale;
        let mut suf = suffix;
        let mut tenth = (bytes * 10 + sc - 1) / sc;
        if tenth >= 100 && exp + 1 < units.len() {
            sc = scale * base;
            suf = units[exp + 1];
            tenth = (bytes * 10 + sc - 1) / sc;
        }
        return format!("{}.{:01}{}", tenth / 10, tenth % 10, suf);
    }
    let mut sc = scale;
    let mut suf = suffix;
    let mut whole = (bytes + sc - 1) / sc;
    if whole >= base && exp + 1 < units.len() {
        sc = scale * base;
        suf = units[exp + 1];
        whole = (bytes + sc - 1) / sc;
        if whole < 10 {
            let tenth = (bytes * 10 + sc - 1) / sc;
            return format!("{}.{:01}{}", tenth / 10, tenth % 10, suf);
        }
    }
    format!("{whole}{suf}")
}

/// block_size 原始參數字串 → 表頭標籤（"M"→"1M"、"512"→"512B"、"1K"→"1K"）
/// uutils 表頭標籤規則（實測）：
/// 1024 鏈可整除 → "4K"/"1M"（不附 B）；1000 鏈可整除 → "1kB"（附 B）；
/// 其他 → SI human（ceil 一位小數，如 1536→"1.6kB"）；<1000 → "512B"。
fn label_from_size(n: u64) -> String {
    const BIN: &[&str] = &["K", "M", "G", "T", "P", "E", "Z", "Y"];
    const SI: &[&str] = &["k", "M", "G", "T", "P", "E", "Z", "Y"];
    let mut d = 1024u64;
    for u in BIN {
        if n % d == 0 && n / d < 1024 {
            return format!("{}{}", n / d, u);
        }
        d = match d.checked_mul(1024) {
            Some(x) => x,
            None => break,
        };
    }
    let mut d = 1000u64;
    for u in SI {
        if n % d == 0 && n / d < 1000 {
            return format!("{}{}B", n / d, u);
        }
        d = match d.checked_mul(1000) {
            Some(x) => x,
            None => break,
        };
    }
    let mut d = 1000u64;
    for u in SI {
        if n >= d && n / d < 1000 {
            let tenth = (n as u128 * 10 + d as u128 - 1) / d as u128;
            return format!("{}.{:01}{}B", tenth / 10, tenth % 10, u);
        }
        d = match d.checked_mul(1000) {
            Some(x) => x,
            None => break,
        };
    }
    format!("{n}B")
}

fn parse_size(s: &str) -> Option<u64> {
    let start = match s.find(|c: char| !c.is_ascii_digit()) {
        Some(k) => k,
        None => {
            // 純數字
            if s.is_empty() {
                return None;
            }
            return s.parse().ok();
        }
    };
    // 以單位開頭（如 -BM → "M"）：數值預設 1
    if start == 0 {
        let mult = unit_mult(s)?;
        return Some(mult);
    }
    let num: u64 = s[..start].parse().ok()?;
    let unit = &s[start..];
    num.checked_mul(unit_mult(unit)?)
}

fn unit_mult(unit: &str) -> Option<u64> {
    Some(match unit {
        "" => 1,
        "K" | "KiB" => 1024,
        "M" | "MiB" => 1024 * 1024,
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
        other => {
            // b/B 後綴（如 512B）與純數字已處理；其他視為無效
            if other == "B" {
                1
            } else {
                return None;
            }
        }
    })
}

fn print_table<W: Write + ?Sized>(
    entries: &[Entry],
    unit: u64,
    unit_label: &str,
    human: u8,
    inodes: bool,
    portability: bool,
    print_type: bool,
    total: bool,
    fields: Option<Vec<String>>,
    mount_source_lens: &[usize],
    stdout: &mut W,
) {
    if entries.is_empty() {
        // 參考行為：沒有任何列時連表頭都不印（df /nonexist → 只有錯誤行）
        return;
    }
    // 決定欄位
    let fields = fields.unwrap_or_else(|| {
        let mut f = vec!["source".to_string()];
        if print_type {
            f.push("fstype".to_string());
        }
        if inodes {
            f.extend(["itotal".into(), "iused".into(), "iavail".into(), "ipcent".into()]);
        } else {
            f.extend(["size".into(), "used".into(), "avail".into(), "pcent".into()]);
        }
        f.push("target".to_string());
        f
    });
    let header: Vec<String> = fields
        .iter()
        .map(|f| match f.as_str() {
            "source" => "Filesystem".to_string(),
            "fstype" => "Type".to_string(),
            "size" if human > 0 => "Size".to_string(),
            "size" if portability => format!("{unit}-blocks"),
            "size" => format!("{unit_label}-blocks"),
            "used" => "Used".to_string(),
            "avail" if human > 0 => "Avail".to_string(),
            "avail" => "Available".to_string(),
            "pcent" if human > 0 => "Use%".to_string(),
            "pcent" if portability => "Capacity".to_string(),
            "pcent" => "Use%".to_string(),
            "itotal" => "Inodes".to_string(),
            "iused" => "IUsed".to_string(),
            "iavail" => "IFree".to_string(),
            "ipcent" => "IUse%".to_string(),
            "target" => "Mounted on".to_string(),
            other => other.to_string(),
        })
        .collect();
    let right: Vec<bool> = fields
        .iter()
        .map(|f| !matches!(f.as_str(), "source" | "fstype" | "target"))
        .collect();

    // 每個 entry 的字串列
    let mut rows: Vec<Vec<String>> = Vec::new();
    let mut tot = (0u128, 0u128, 0u128);
    let mut tot_raw = (0u64, 0u64); // (used, avail) 原始盤區塊
    let mut tot_bytes = (0u128, 0u128, 0u128); // (size, used, avail) 位元組
    for e in entries {
        let mut row = Vec::new();
        for (fi, f) in fields.iter().enumerate() {
            let _ = fi;
            match f.as_str() {
                "source" => row.push(e.source.clone()),
                "fstype" => row.push(e.fstype.clone()),
                "target" => row.push(e.target.clone()),
                "size" | "used" | "avail" => {
                    if human > 0 {
                        // 人類可讀一律以**原始位元組數**換算（不受 -B 換算取整影響）
                        let frsize = if e.st.f_frsize != 0 { e.st.f_frsize } else { e.st.f_bsize };
                        let bytes = match f.as_str() {
                            "size" => e.st.f_blocks as u128 * frsize as u128,
                            "used" => (e.st.f_blocks - e.st.f_bfree) as u128 * frsize as u128,
                            _ => e.st.f_bavail as u128 * frsize as u128,
                        };
                        row.push(humanize(bytes, human == 2));
                    } else {
                        let (s, u, a) = e.blocks(unit);
                        let v = match f.as_str() {
                            "size" => s,
                            "used" => u,
                            _ => a,
                        };
                        row.push(v.to_string());
                    }
                }
                "pcent" => {
                    // 參考行為：Use% 以原始磁盤區塊數計算，不受 -B 換算取整影響
                    let used = e.st.f_blocks - e.st.f_bfree;
                    let avail = e.st.f_bavail;
                    row.push(if used + avail == 0 { "-".to_string() } else { format!("{}%", pct(used, avail)) });
                }
                "itotal" | "iused" | "iavail" => {
                    let (s, u, a) = e.inos();
                    let v = match f.as_str() {
                        "itotal" => s,
                        "iused" => u,
                        _ => a,
                    };
                    row.push(v.to_string());
                }
                "ipcent" => {
                    let (s, u, a) = e.inos();
                    let _ = s;
                    row.push(if u + a == 0 { "-".to_string() } else { format!("{}%", pct(u, a)) });
                }
                _ => row.push(String::new()),
            }
        }
        let (s, u, a) = e.blocks(unit);
        tot = (tot.0 + s as u128, tot.1 + u as u128, tot.2 + a as u128);
        tot_raw = (tot_raw.0 + (e.st.f_blocks - e.st.f_bfree), tot_raw.1 + e.st.f_bavail);
        let frs = if e.st.f_frsize != 0 { e.st.f_frsize } else { e.st.f_bsize };
        tot_bytes = (
            tot_bytes.0 + e.st.f_blocks as u128 * frs as u128,
            tot_bytes.1 + (e.st.f_blocks - e.st.f_bfree) as u128 * frs as u128,
            tot_bytes.2 + e.st.f_bavail as u128 * frs as u128,
        );
        rows.push(row);
    }
    if total {
        let mut row = Vec::new();
        for f in &fields {
            match f.as_str() {
                "source" => row.push("total".to_string()),
                "target" => row.push("-".to_string()),
                "fstype" => row.push("-".to_string()),
                "size" | "used" | "avail" => {
                    if human > 0 {
                        let v = match f.as_str() {
                            "size" => tot_bytes.0,
                            "used" => tot_bytes.1,
                            _ => tot_bytes.2,
                        };
                        row.push(humanize(v, human == 2));
                    } else {
                        let v = match f.as_str() {
                            "size" => tot.0,
                            "used" => tot.1,
                            _ => tot.2,
                        };
                        row.push(v.to_string());
                    }
                }
                "pcent" => row.push(if tot_raw.0 + tot_raw.1 == 0 { "-".into() } else { format!("{}%", pct(tot_raw.0, tot_raw.1)) }),
                "itotal" | "iused" | "iavail" | "ipcent" => row.push(String::new()),
                _ => row.push(String::new()),
            }
        }
        rows.push(row);
    }

    // 欄寬：數字欄 = max(欄位名, 各位數)，右對齊；文字欄左對齊，單空格分隔
    let mut widths = vec![0usize; fields.len()];
    for (i, h) in header.iter().enumerate() {
        widths[i] = widths[i].max(h.len());
        for r in &rows {
            widths[i] = widths[i].max(r[i].len());
        }
    }
    // 參考 quirk：human 模式下 size/used/avail 欄最小寬 5（pcent 不受影響）
    if human > 0 {
        for (i, f) in fields.iter().enumerate() {
            if matches!(f.as_str(), "size" | "used" | "avail") {
                widths[i] = widths[i].max(5);
            }
        }
    }
    // 參考 quirk：Filesystem 欄寬取整張掛載表的最長來源名（即使被過濾/未列出）
    if !fields.is_empty() && fields[0] == "source" {
        for l in mount_source_lens {
            widths[0] = widths[0].max(*l);
        }
    }
    let mut line = String::new();
    for (i, h) in header.iter().enumerate() {
        if right[i] {
            line.push_str(&format!("{:>w$} ", h, w = widths[i]));
        } else {
            line.push_str(&format!("{:<w$} ", h, w = widths[i]));
        }
    }
    let _ = writeln!(stdout, "{}", line.trim_end());
    for r in &rows {
        let mut line = String::new();
        for (i, c) in r.iter().enumerate() {
            if right[i] {
                line.push_str(&format!("{:>w$} ", c, w = widths[i]));
            } else {
                line.push_str(&format!("{:<w$} ", c, w = widths[i]));
            }
        }
        let _ = writeln!(stdout, "{}", line.trim_end());
    }
}
