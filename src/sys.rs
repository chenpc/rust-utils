//! 對 libc 的最小 FFI：termios（raw mode）、mount、finit_module、reboot、isatty。
//!
//! 不使用外部 crate；crt-static 連線時仍會連結 glibc，所以這些符號都在。
//! x86_64 Linux/glibc 的佈局與常數（termios NCCS=32）。

use std::ffi::CString;
use std::io;
use std::io::Write;

#[derive(Clone, Copy)]
#[repr(C)]
pub struct Termios {
    pub c_iflag: u32,
    pub c_oflag: u32,
    pub c_cflag: u32,
    pub c_lflag: u32,
    pub c_line: u8,
    pub c_cc: [u8; 32],
    pub c_ispeed: u32,
    pub c_ospeed: u32,
}

extern "C" {
    pub fn tcgetattr(fd: i32, t: *mut Termios) -> i32;
    pub fn tcsetattr(fd: i32, action: i32, t: *const Termios) -> i32;
    pub fn isatty(fd: i32) -> i32;
    pub fn mount(
        source: *const i8,
        target: *const i8,
        fstype: *const i8,
        flags: u64,
        data: *const i8,
    ) -> i32;
    pub fn reboot(cmd: i32) -> i32;
    pub fn syscall(num: i64, ...) -> i64;
}

pub const STDIN_FD: i32 = 0;
pub const STDOUT_FD: i32 = 1;

// tcsetattr actions
pub const TCSANOW: i32 = 0;
// c_lflag bits
const ICANON: u32 = 0o0002;
const ECHO: u32 = 0o0010;
const ISIG: u32 = 0o0001;
// c_cc indices
const VTIME: usize = 5;
const VMIN: usize = 6;

/// 進入 raw mode（交還給呼叫端還原）。失敗回原值（非 tty 時呼叫端應回退）。
pub fn raw_mode(fd: i32) -> Option<Termios> {
    let mut saved = Termios {
        c_iflag: 0,
        c_oflag: 0,
        c_cflag: 0,
        c_lflag: 0,
        c_line: 0,
        c_cc: [0; 32],
        c_ispeed: 0,
        c_ospeed: 0,
    };
    if unsafe { tcgetattr(fd, &mut saved) } != 0 {
        return None;
    }
    let mut raw = saved;
    raw.c_lflag &= !(ICANON | ECHO | ISIG);
    raw.c_cc[VTIME] = 0;
    raw.c_cc[VMIN] = 1;
    if unsafe { tcsetattr(fd, TCSANOW, &raw) } != 0 {
        return None;
    }
    Some(saved)
}

pub fn restore_mode(fd: i32, saved: &Termios) {
    unsafe {
        tcsetattr(fd, TCSANOW, saved);
    }
}

pub fn stdin_is_tty() -> bool {
    unsafe { isatty(STDIN_FD) == 1 }
}

fn cstr(s: &str) -> io::Result<CString> {
    CString::new(s).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "interior NUL"))
}

/// mount(2) 薄封裝。
pub fn mount_fs(source: &str, target: &str, fstype: &str, data: &str) -> io::Result<()> {
    let (s, t, f, d) = (cstr(source)?, cstr(target)?, cstr(fstype)?, cstr(data)?);
    if unsafe {
        mount(s.as_ptr(), t.as_ptr(), f.as_ptr(), 0, if data.is_empty() { std::ptr::null() } else { d.as_ptr() })
    } != 0
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// finit_module(fd)：從已開好的 .ko 檔案載入核心模組。
pub fn finit_module(fd: i32) -> io::Result<()> {
    let empty = CString::new("").unwrap();
    // SYS_finit_module = 312 (x86_64)
    let rc = unsafe { syscall(312, fd as i64, empty.as_ptr() as i64, 0u64 as i64) };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// init_module(image, len, params)：SYS_init_module = 175 (x86_64)。/// 本 guest kernel 上 finit_module 回 ESRCH（實測），busybox insmod 用的是這條路徑。
pub fn init_module(image: &[u8]) -> io::Result<()> {
    let empty = CString::new("").unwrap();
    let rc = unsafe {
        syscall(
            175,
            image.as_ptr() as i64,
            image.len() as i64,
            empty.as_ptr() as i64,
        )
    };
    if rc != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

pub const RB_POWER_OFF: i32 = 0x4321fed2;

// ---- 磁碟/系統（F 群）：sync / syncfs / statvfs ----

/// glibc x86_64 statvfs 佈局（全 64-bit）。
#[repr(C)]
pub struct Statvfs {
    pub f_bsize: u64,
    pub f_frsize: u64,
    pub f_blocks: u64,
    pub f_bfree: u64,
    pub f_bavail: u64,
    pub f_files: u64,
    pub f_ffree: u64,
    pub f_favail: u64,
    pub f_fsid: u64,
    pub f_flag: u64,
    pub f_namemax: u64,
}

extern "C" {
    pub fn sync();
    pub fn syncfs(fd: i32) -> i32;
    pub fn statvfs64(path: *const i8, buf: *mut Statvfs) -> i32;
    pub fn stat(path: *const i8, buf: *mut StatC) -> i32;
}

/// x86_64 struct stat（只需前兩欄即可取 st_dev）
#[repr(C)]
pub struct StatC {
    pub st_dev: u64,
    pub st_ino: u64,
    pub st_nlink: u64,
    pub st_mode: u32,
    pub st_uid: u32,
    pub st_gid: u32,
    pub st_pad0: u32,
    pub st_rdev: u64,
    pub st_size: i64,
    pub st_blksize: i64,
    pub st_blocks: i64,
    pub st_atime: i64,
    pub st_atime_nsec: u64,
    pub st_mtime: i64,
    pub st_mtime_nsec: u64,
    pub st_ctime: i64,
    pub st_ctime_nsec: u64,
    pub unused: [i64; 3],
}

/// 取 path 所在裝置號（GNU df 的去重鍵）；失敗回 None。
pub fn stat_dev(path: &str) -> Option<u64> {
    let p = cstr(path).ok()?;
    let mut st = unsafe { std::mem::zeroed::<StatC>() };
    if unsafe { stat(p.as_ptr(), &mut st) } != 0 {
        return None;
    }
    Some(st.st_dev)
}

/// sync(2)：全域 flush。
pub fn sync_all() {
    unsafe { sync() }
}

/// syncfs(fd)：只 flush 該 fd 所在檔案系統。
pub fn sync_fs(fd: i32) -> io::Result<()> {
    if unsafe { syncfs(fd) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// statvfs(2) 封裝。
pub fn statvfs(path: &str) -> io::Result<Statvfs> {
    let p = cstr(path)?;
    let mut st = Statvfs {
        f_bsize: 0,
        f_frsize: 0,
        f_blocks: 0,
        f_bfree: 0,
        f_bavail: 0,
        f_files: 0,
        f_ffree: 0,
        f_favail: 0,
        f_fsid: 0,
        f_flag: 0,
        f_namemax: 0,
    };
    if unsafe { statvfs64(p.as_ptr(), &mut st) } != 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(st)
}

extern "C" {
    #[link_name = "mknod"]
    fn mknod_c(path: *const i8, mode: u32, dev: u64) -> i32;
}

/// libc mknod 封裝（cp 重建 FIFO 用）；0 成功，-1 + errno。
pub fn mknod(path: &str, mode: u32, dev: u64) -> i64 {
    let c = match std::ffi::CString::new(path) {
        Ok(c) => c,
        Err(_) => return -1,
    };
    unsafe { mknod_c(c.as_ptr(), mode, dev) as i64 }
}

/// pipe(2) 包裝：回 (read_end, write_end)。本工具鏈無 std::os::unix::pipe。
pub fn anon_pipe() -> io::Result<(std::os::fd::OwnedFd, std::os::fd::OwnedFd)> {
    extern "C" {
        fn pipe(fds: *mut i32) -> i32;
    }
    let mut fds = [0i32; 2];
    if unsafe { pipe(fds.as_mut_ptr()) } != 0 {
        return Err(io::Error::last_os_error());
    }
    use std::os::fd::FromRawFd;
    Ok(unsafe { (std::os::fd::OwnedFd::from_raw_fd(fds[0]), std::os::fd::OwnedFd::from_raw_fd(fds[1])) })
}

/// poweroff(8)：直接 syscall reboot(169)（glibc reboot() 在本 guest 回 ENOSYS，實測）。
/// 失敗時回 errno，呼叫者決定後續。
pub fn power_off() -> io::Result<()> {
    // linux/reboot.h：MAGIC1=0xfee1dead MAGIC2=28121857
    let mut last = 0;
    for magic2 in [28121857_i64, 672274793, 3121347620, 672274774] {
        let rc = unsafe {
            syscall(
                169,
                0xfee1dead_i64,
                magic2,
                RB_POWER_OFF as i64,
                std::ptr::null::<i8>() as i64,
            )
        };
        if rc == 0 {
            std::process::exit(0);
        }
        last = io::Error::last_os_error().raw_os_error().unwrap_or(0);
    }
    Err(io::Error::from_raw_os_error(last))
}

/// 後備關機路徑：sysrq（部分 kernel 對 reboot(2) 有額外限制，本 guest kernel 實測 EINVAL）。
pub fn power_off_sysrq() {
    for key in ["o", "b"] {
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .write(true)
            .open("/proc/sysrq-trigger")
        {
            let _ = f.write_all(key.as_bytes());
            let _ = f.flush();
        }
    }
}

// ---- du: 本地時間格式化（--time-style）----
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct Tm {
    pub tm_sec: i32,
    pub tm_min: i32,
    pub tm_hour: i32,
    pub tm_mday: i32,
    pub tm_mon: i32,
    pub tm_year: i32,
    pub tm_wday: i32,
    pub tm_yday: i32,
    pub tm_isdst: i32,
    pub tm_gmtoff: i64,
    pub tm_zone: *const i8,
}

extern "C" {
    pub fn localtime_r(timep: &i64, result: &mut Tm) -> *mut Tm;
}

/// 將 unix 時間戳轉為本地 Tm；失敗回 None。
pub fn local_tm(secs: i64) -> Option<Tm> {
    let mut tm = Tm::default();
    if unsafe { localtime_r(&secs, &mut tm) }.is_null() {
        None
    } else {
        Some(tm)
    }
}

extern "C" {
    pub fn lseek(fd: i32, offset: i64, whence: i32) -> i64;
}

/// 安全包裝：回傳 Option<u64> 位置（ESPIPE 等失敗回 None）。
pub fn lseek_pos(fd: i32, off: i64, whence: i32) -> Option<u64> {
    let r = unsafe { lseek(fd, off, whence) };
    if r < 0 {
        None
    } else {
        Some(r as u64)
    }
}

// ---------------------------------------------------------------------------
// id / whoami / groups / logname / hostid 使用的 libc FFI
// ---------------------------------------------------------------------------

extern "C" {
    #[link_name = "geteuid"]
    fn c_geteuid() -> u32;
    #[link_name = "getuid"]
    fn c_getuid() -> u32;
    #[link_name = "getgid"]
    fn c_getgid() -> u32;
    #[link_name = "getegid"]
    fn c_getegid() -> u32;
    #[link_name = "getgroups"]
    fn c_getgroups(size: i32, list: *mut u32) -> i32;
    #[link_name = "getlogin"]
    fn c_getlogin() -> *const i8;
    #[link_name = "gethostid"]
    fn c_gethostid() -> i32;
}

// 呼叫端一律用這些安全包裝（extern 本身是 unsafe）。
pub fn getuid() -> u32 {
    unsafe { c_getuid() }
}

pub fn geteuid() -> u32 {
    unsafe { c_geteuid() }
}

pub fn getgid() -> u32 {
    unsafe { c_getgid() }
}

pub fn getegid() -> u32 {
    unsafe { c_getegid() }
}

pub fn gethostid_i32() -> i32 {
    unsafe { c_gethostid() }
}

/// 目前程序的補充群組清單（getgroups(2)）；失敗回空。
pub fn process_groups() -> Vec<u32> {
    unsafe {
        let n = c_getgroups(0, std::ptr::null_mut());
        if n <= 0 {
            return Vec::new();
        }
        let n = (n as usize).min(4096);
        let mut buf = vec![0u32; n];
        let m = c_getgroups(n as i32, buf.as_mut_ptr());
        if m < 0 {
            Vec::new()
        } else {
            buf.truncate(m as usize);
            buf
        }
    }
}

/// getlogin(3)；無 login name 回 None。
pub fn login_name() -> Option<String> {
    unsafe {
        let p = c_getlogin();
        if p.is_null() {
            return None;
        }
        let mut v: Vec<u8> = Vec::new();
        let mut i = 0usize;
        loop {
            let c = *(p.add(i));
            if c == 0 {
                break;
            }
            v.push(c as u8);
            i += 1;
            if i > 4096 {
                break;
            }
        }
        String::from_utf8(v).ok()
    }
}
