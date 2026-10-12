use coreutils::testutil as t;
use std::path::PathBuf;


pub fn rec(utype: i16, pid: i32, line: &str, id: &str, user: &str, host: &str, tv_sec: i32) -> Vec<u8> {
    rec_full(utype, pid, line, id, user, host, tv_sec, 0, 0)
}

pub fn rec_full(utype: i16, pid: i32, line: &str, id: &str, user: &str, host: &str, tv_sec: i32, term: i16, exit: i16) -> Vec<u8> {
    let mut b = vec![0u8; 384];
    b[0..2].copy_from_slice(&utype.to_le_bytes());
    b[4..8].copy_from_slice(&pid.to_le_bytes());
    fn put(b: &mut [u8], at: usize, s: &str, cap: usize) {
        let raw = s.as_bytes();
        let n = raw.len().min(cap - 1);
        b[at..at + n].copy_from_slice(&raw[..n]);
    }
    put(&mut b, 8, line, 32);
    put(&mut b, 40, id, 4);
    put(&mut b, 44, user, 32);
    put(&mut b, 76, host, 256);
    b[332..334].copy_from_slice(&term.to_le_bytes());
    b[334..336].copy_from_slice(&exit.to_le_bytes());
    b[340..344].copy_from_slice(&tv_sec.to_le_bytes());
    b
}

pub fn write_utmp(path: &std::path::Path, records: &[Vec<u8>]) {
    let mut data = Vec::new();
    for r in records {
        data.extend_from_slice(r);
    }
    std::fs::write(path, data).unwrap();
}

pub fn ts(secs: i32) -> String {
    let (y, mo, d, h, mi) = coreutils::sys::local_time(secs as i64).unwrap();
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, d, h, mi)
}

fn fresh(tag: &str) -> PathBuf {
    let p = PathBuf::from(format!("/tmp/users_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

#[test]
fn lists_logged_in_users_sorted_with_duplicates() {
    let d = fresh("sort");
    let f = d.join("utmp");
    write_utmp(
        &f,
        &[
            rec(7, 100, "pts/1", "pts1", "zoe", "h1", 1700000000),
            rec(7, 101, "pts/2", "pts2", "amy", "h2", 1700000001),
            rec(7, 102, "pts/3", "pts3", "zoe", "h3", 1700000002),
        ],
    );
    let r = t::run("users", &[f.to_str().unwrap()], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "amy zoe zoe\n");
}

#[test]
fn only_user_process_records_count() {
    let d = fresh("types");
    let f = d.join("utmp");
    write_utmp(
        &f,
        &[
            rec(7, 1, "pts/0", "p0", "alice", "", 1700000000),
            rec(6, 2, "tty1", "t1", "LOGIN", "", 1700000000),
            rec(5, 3, "~~", "id1", "init", "", 1700000000),
            rec(8, 4, "pts/9", "p9", "gone", "", 1700000000),
            rec(2, 0, "~~", "", "", "", 1700000000),
            rec(1, 0, "~~", "5", "", "", 1700000000),
        ],
    );
    let r = t::run("users", &[f.to_str().unwrap()], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "alice\n");
}

#[test]
fn empty_and_missing_files_are_not_an_error() {
    let d = fresh("empty");
    let f = d.join("utmp");
    std::fs::write(&f, b"").unwrap();
    let r = t::run("users", &[f.to_str().unwrap()], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "");
    let r = t::run("users", &["/nonexistent-utmp-file"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "");
    assert_eq!(r.err(), "");
}

#[test]
fn partial_trailing_record_is_ignored() {
    let d = fresh("partial");
    let f = d.join("utmp");
    let mut data = rec(7, 1, "pts/0", "p0", "alice", "", 1700000000);
    data.extend_from_slice(&[7u8, 0, 0, 0]);
    std::fs::write(&f, data).unwrap();
    let r = t::run("users", &[f.to_str().unwrap()], "");
    assert_eq!(r.out(), "alice\n");
}

#[test]
fn help_and_version() {
    let r = t::run("users", &["--help"], "");
    assert_eq!(
        r.out(),
        "Print the user names of users currently logged in to the current host.\n\nUsage: users [FILE]\n\nArguments:\n  [file]  \n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version\n"
    );
    assert_eq!(r.code, 0);
    assert_eq!(t::run("users", &["--version"], "").out(), "users (uutils coreutils) 0.10.0\n");
    assert_eq!(t::run("users", &["-V"], "").out(), "users (uutils coreutils) 0.10.0\n");
    assert_eq!(t::run("users", &["-h"], "").out(), t::run("users", &["--help"], "").out());
    assert_eq!(t::run("users", &["-h", "-h"], "").out(), t::run("users", &["--help"], "").out());
}

#[test]
fn clap_style_errors() {
    let r = t::run("users", &["--bogus"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: unexpected argument '--bogus' found\n\n  tip: to pass '--bogus' as a value, use '-- --bogus'\n\nUsage: users [FILE]\n\nFor more information, try '--help'.\n"
    );
    let r = t::run("users", &["-x"], "");
    assert_eq!(
        r.err(),
        "error: unexpected argument '-x' found\n\n  tip: to pass '-x' as a value, use '-- -x'\n\nUsage: users [FILE]\n\nFor more information, try '--help'.\n"
    );
    let d = fresh("two");
    let a = d.join("a");
    let b = d.join("b");
    std::fs::write(&a, b"").unwrap();
    std::fs::write(&b, b"").unwrap();
    let r = t::run("users", &[a.to_str().unwrap(), b.to_str().unwrap()], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        format!(
            "error: unexpected argument '{}' found\n\nUsage: users [FILE]\n\nFor more information, try '--help'.\n",
            b.display()
        )
    );
}

#[test]
fn double_dash_forces_operand_and_dispatch_via_binary() {
    let d = fresh("dd");
    let f = d.join("utmp");
    write_utmp(&f, &[rec(7, 1, "pts/0", "p0", "bob", "", 1700000000)]);
    let r = t::run("users", &["--", f.to_str().unwrap()], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "bob\n");
    let out = t::run_bin("users", &[f.to_str().unwrap()], "");
    assert_eq!(out.out(), "bob\n");
}
