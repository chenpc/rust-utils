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

/// init_module(image, len, params)：SYS_init_module = 175 (x86_64)。
/// 本 guest kernel 上 finit_module 回 ESRCH（實測），busybox insmod 用的是這條路徑。
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
