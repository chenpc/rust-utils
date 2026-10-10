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

pub fn run(args: &[String], _stdin: crate::In, stdout: crate::Out, stderr: crate::Out) -> i32 {
    let mut follow = false;
    let mut terse = false;
    let mut file_system = false;
    let mut format: Option<String> = None;
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
                "dereference" => follow = true,
                "terse" => terse = true,
                "file-system" => file_system = true,
                "format" => match getval(&mut i) {
                    Some(v) => {
                        format = Some(v);
                        terse = false;
                    }
                    None => {
                        writeln!(stderr, "stat: option requires an argument -- 'format'").ok();
                        return 2;
                    }
                },
                "help" => {
                    writeln!(stdout, "Usage: stat [OPTION]... FILE...\nDisplay file or file system status").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "stat (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                _ => {
                    writeln!(stderr, "stat: unrecognized option '{}'", a).ok();
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
                    'L' => follow = true,
                    'f' => file_system = true,
                    't' | 'd' => terse = true,
                    'c' => {
                        let rest: String = chars[k + 1..].iter().collect();
                        let val = if !rest.is_empty() {
                            Some(rest)
                        } else {
                            i += 1;
                            args.get(i).cloned()
                        };
                        match val {
                            Some(v) => {
                                format = Some(v);
                                terse = false;
                            }
                            None => {
                                writeln!(stderr, "stat: option requires an argument -- 'c'").ok();
                                return 2;
                            }
                        }
                        k = chars.len();
                        continue;
                    }
                    c => {
                        writeln!(stderr, "stat: invalid option -- '{}'", c).ok();
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
        writeln!(stderr, "stat: missing operand").ok();
        return 2;
    }
    let mut rc = 0;
    for f in &files {
        if file_system {
            rc |= print_fs(f, format.as_deref(), stdout, stderr);
            continue;
        }
        if let Some(fmt) = &format {
            match statx(f, follow) {
                Ok(st) => {
                    let out = expand(fmt, f, &st);
                    write!(stdout, "{}\n", out).ok();
                }
                Err(e) => {
                    writeln!(stderr, "stat: cannot statx '{}': {}", f, crate::pathname::io_reason_pub(&e)).ok();
                    rc = 1;
                }
            }
            continue;
        }
        match statx(f, follow) {
            Ok(st) => {
                if terse {
                    let t = expand("%n %s %b %f %u %g %D %i %h %t %T %X %Y %Z %W %o", f, &st);
                    writeln!(stdout, "{}", t).ok();
                } else {
                    print_default(f, &st, stdout);
                }
            }
            Err(e) => {
                writeln!(stderr, "stat: cannot statx '{}': {}", f, crate::pathname::io_reason_pub(&e)).ok();
                rc = 1;
            }
        }
    }
    rc
}

fn mode_bits(st: &Statx) -> u32 {
    st.stx_mode & 0o170000
}

fn type_str(st: &Statx) -> &'static str {
    match mode_bits(st) & 0o170000 {
        0o010000 => "FIFO",
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
            Ok(t) => writeln!(stdout, "  File: '{}' -> '{}'", path, t.display()).ok(),
            Err(_) => writeln!(stdout, "  File: {}", path).ok(),
        };
    } else {
        writeln!(stdout, "  File: {}", path).ok();
    }
    write!(stdout, "  Size: {:<10}\tBlocks: {:<11}IO Block: {:<4}   {}\n", st.stx_size, st.stx_blocks, st.stx_blksize, type_desc(st)).ok();
    write!(
        stdout,
        "Device: {},{}\tInode: {:<11} Links: {}\n",
        st.stx_dev_major,
        st.stx_dev_minor,
        st.stx_ino,
        st.stx_nlink
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

fn expand(fmt: &str, path: &str, st: &Statx) -> String {
    let b = fmt.as_bytes();
    let mut out = String::new();
    let mut i = 0usize;
    while i < b.len() {
        if b[i] == b'%' && i + 1 < b.len() {
            i += 1;
            let d = b[i] as char;
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
                'd' => format!("{:x}", makedev(st.stx_dev_major, st.stx_dev_minor)),
                'D' => format!("{:x}", makedev(st.stx_dev_major, st.stx_dev_minor)),
                'H' => "?".to_string(),
                'T' => "0".to_string(),
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
                },
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
                'C' => "?".to_string(),
                '%' => "%".to_string(),
                other => format!("%{}", other),
            };
            out.push_str(&v);
        } else {
            out.push(b[i] as char);
        }
        i += 1;
    }
    out
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

fn print_fs(path: &str, format: Option<&str>, stdout: &mut dyn Write, stderr: &mut dyn Write) -> i32 {
    let c = match CString::new(path) {
        Ok(c) => c,
        Err(_) => {
            writeln!(stderr, "stat: cannot read file system information for '{}': Invalid argument", path).ok();
            return 1;
        }
    };
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
        let e = std::io::Error::last_os_error();
        writeln!(stderr, "stat: cannot read file system information for '{}': {}", path, crate::pathname::io_reason_pub(&e)).ok();
        return 1;
    }
    if let Some(fmt) = format {
        let b = fmt.as_bytes();
        let mut out = String::new();
        let mut i = 0usize;
        while i < b.len() {
            if b[i] == b'%' && i + 1 < b.len() {
                i += 1;
                let d = b[i] as char;
                let v: String = match d {
                    'a' => fs.f_bavail.to_string(),
                    'b' => fs.f_blocks.to_string(),
                    'c' => fs.f_files.to_string(),
                    'd' => fs.f_ffree.to_string(),
                    'f' => fs.f_bfree.to_string(),
                    'i' => format!("{:x}", ((fs.f_fsid[0] as u32 as u64) << 32) | fs.f_fsid[1] as u32 as u64),
                    'l' => fs.f_namelen.to_string(),
                    'n' => path.to_string(),
                    's' => (fs.f_bsize as u64).to_string(),
                    'S' => fs.f_frsize.to_string(),
                    'T' => {
                        let mut name = format!("0x{:x}", fs.f_type as u64);
                        for (magic, mname) in FS_MAGICS {
                            if fs.f_type as u64 == *magic {
                                name = mname.to_string();
                                break;
                            }
                        }
                        name
                    }
                    'p' => "?".to_string(),
                    '%' => "%".to_string(),
                    other => format!("%{}", other),
                };
                out.push_str(&v);
            } else {
                out.push(b[i] as char);
            }
            i += 1;
        }
        writeln!(stdout, "{}", out).ok();
        return 0;
    }
    let fsid = ((fs.f_fsid[0] as u32 as u64) << 32) | fs.f_fsid[1] as u32 as u64;
    writeln!(stdout, "  File: \"{}\"", path).ok();
    writeln!(
        stdout,
        "    ID: {:x} Namelen: {:<7} Type: {}",
        fsid,
        fs.f_namelen,
        fs_name(fs.f_type as u64)
    )
    .ok();
    writeln!(stdout, "Block Size: {:<10} Fundamental block size: {}", fs.f_bsize, fs.f_frsize).ok();
    writeln!(
        stdout,
        "Blocks: Total: {:<10} Free: {:<10} Available: {}",
        fs.f_blocks, fs.f_bfree, fs.f_bavail
    )
    .ok();
    writeln!(stdout, "Inodes: Total: {:<10} Free: {}", fs.f_files, fs.f_ffree).ok();
    0
}

fn fs_name(t: u64) -> String {
    for (magic, name) in FS_MAGICS {
        if t == *magic {
            return name.to_string();
        }
    }
    format!("0x{:x}", t)
}

// silence unused import warnings when MetadataExt path changes
#[allow(dead_code)]
fn _unused(m: &std::fs::Metadata) -> u64 {
    m.dev()
}
