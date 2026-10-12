//! Minimal reader for the Linux utmp record format.
//!
//! The reference implementation reaches for libsystemd's `sd_utmp` when it is
//! available, which is not an option for a static binary with no init system in
//! the guest.  Both `users` and `who` therefore parse the record file directly.
//!
//! The layout is the glibc `struct utmp` as used on x86_64 (384 bytes per
//! record, little endian, fixed size character arrays terminated by NUL).

pub const RECORD_SIZE: usize = 384;

pub const RUN_LVL: i16 = 1;
pub const BOOT_TIME: i16 = 2;
pub const NEW_TIME: i16 = 3;
pub const OLD_TIME: i16 = 4;
pub const INIT_PROCESS: i16 = 5;
pub const LOGIN_PROCESS: i16 = 6;
pub const USER_PROCESS: i16 = 7;
pub const DEAD_PROCESS: i16 = 8;

pub const DEFAULT_PATH: &str = "/var/run/utmp";

#[derive(Debug, Clone)]
pub struct Record {
    pub utype: i16,
    pub pid: i32,
    pub line: String,
    pub id: String,
    pub user: String,
    pub host: String,
    pub exit_status: i16,
    pub term_status: i16,
    pub session: i32,
    pub tv_sec: i32,
    pub tv_usec: i32,
    pub addr_v6: [i32; 4],
}

fn field(data: &[u8], start: usize, len: usize) -> String {
    let raw = &data[start..start + len];
    let end = raw.iter().position(|b| *b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
}

fn i16_at(data: &[u8], at: usize) -> i16 {
    i16::from_le_bytes([data[at], data[at + 1]])
}

fn i32_at(data: &[u8], at: usize) -> i32 {
    i32::from_le_bytes([data[at], data[at + 1], data[at + 2], data[at + 3]])
}

fn parse_record(data: &[u8]) -> Record {
    Record {
        utype: i16_at(data, 0),
        pid: i32_at(data, 4),
        line: field(data, 8, 32),
        id: field(data, 40, 4),
        user: field(data, 44, 32),
        host: field(data, 76, 256),
        term_status: i16_at(data, 332),
        exit_status: i16_at(data, 334),
        session: i32_at(data, 336),
        tv_sec: i32_at(data, 340),
        tv_usec: i32_at(data, 344),
        addr_v6: [
            i32_at(data, 348),
            i32_at(data, 352),
            i32_at(data, 356),
            i32_at(data, 360),
        ],
    }
}

/// Read every complete record from `path`.  A missing or unreadable file yields
/// an empty list (the reference reports nothing rather than failing here).
pub fn read_records(path: &str) -> Vec<Record> {
    let data = match std::fs::read(path) {
        Ok(d) => d,
        Err(_) => return Vec::new(),
    };
    data.chunks_exact(RECORD_SIZE)
        .map(parse_record)
        .collect()
}
