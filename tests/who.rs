use coreutils::testutil as t;
use std::path::PathBuf;

pub fn rec(utype: i16, pid: i32, line: &str, id: &str, user: &str, host: &str, tv_sec: i32) -> Vec<u8> {
    rec_full(utype, pid, line, id, user, host, tv_sec, 0, 0)
}

pub fn rec_full(
    utype: i16,
    pid: i32,
    line: &str,
    id: &str,
    user: &str,
    host: &str,
    tv_sec: i32,
    term: i16,
    exit: i16,
) -> Vec<u8> {
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

fn write_utmp(path: &PathBuf, records: &[Vec<u8>]) {
    let mut data = Vec::new();
    for r in records {
        data.extend_from_slice(r);
    }
    std::fs::write(path, data).unwrap();
}

fn ts(secs: i32) -> String {
    let (y, mo, d, h, mi) = coreutils::sys::local_time(secs as i64).unwrap();
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, d, h, mi)
}

fn fresh(tag: &str) -> PathBuf {
    let p = PathBuf::from(format!("/tmp/who_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn file(tag: &str, records: &[Vec<u8>]) -> PathBuf {
    let d = fresh(tag);
    let f = d.join("utmp");
    write_utmp(&f, records);
    f
}

const T: i32 = 1700000000;

#[test]
fn headings_match_the_reference_layout() {
    let f = file("head", &[]);
    let args = f.to_str().unwrap();
    let cases: &[(&[&str], &str)] = &[
        (&["-H"], "NAME     LINE         TIME       COMMENT"),
        (&["-s", "-H"], "NAME     LINE         TIME       COMMENT"),
        (&["-u", "-H"], "NAME     LINE         TIME       IDLE          PID COMMENT"),
        (&["-a", "-H"], "NAME       LINE         TIME       IDLE          PID COMMENT  EXIT"),
        (&["-d", "-H"], "NAME     LINE         TIME       IDLE          PID COMMENT  EXIT"),
        (&["-l", "-H"], "NAME     LINE         TIME       IDLE          PID COMMENT"),
        (&["-r", "-H"], "NAME     LINE         TIME       IDLE          PID COMMENT"),
        (&["-p", "-H"], "NAME     LINE         TIME              PID COMMENT"),
        (&["-b", "-H"], "NAME     LINE         TIME              PID COMMENT"),
        (&["-t", "-H"], "NAME     LINE         TIME              PID COMMENT"),
        (&["-T", "-H"], "NAME       LINE         TIME       COMMENT"),
    ];
    for (opts, want) in cases {
        let mut argv: Vec<&str> = opts.to_vec();
        argv.push(args);
        let r = t::run("who", &argv, "");
        assert_eq!(r.code, 0, "opts {:?}", opts);
        assert_eq!(r.out().trim_end(), *want, "opts {:?}", opts);
    }
}

#[test]
fn short_format_columns() {
    let f = file("short", &[rec(7, 4242, "pts/0", "p0", "alice", "host.example", T)]);
    let r = t::run("who", &[f.to_str().unwrap()], "");
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out(),
        format!("alice    pts/0        {} host.example\n", ts(T))
    );
    // a second record keeps the column alignment
    let f2 = file("short2", &[
        rec(7, 1, "pts/0", "p0", "alice", "", T),
        rec(7, 2, "tty1", "t1", "b", "h", T + 60),
    ]);
    let r = t::run("who", &[f2.to_str().unwrap()], "");
    let out = r.out();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 2);
    assert_eq!(lines[0], format!("alice    pts/0        {} ", ts(T)).trim_end());
    assert_eq!(lines[1], format!("b        tty1         {} h", ts(T + 60)));
}

#[test]
fn record_selection_follows_the_options() {
    let records = vec![
        rec(1, 0, "~~", "5", "", "", T),
        rec(2, 0, "~~", "", "", "", T),
        rec(3, 0, "~~", "", "", "", T),
        rec(4, 0, "~~", "", "", "", T),
        rec(5, 10, "~~", "id1", "init", "", T),
        rec(6, 11, "tty1", "t1", "LOGIN", "", T),
        rec(7, 12, "pts/0", "p0", "alice", "h", T),
        rec(8, 13, "pts/1", "p1", "dead", "", T),
    ];
    let f = file("select", &records);
    let p = f.to_str().unwrap();
    let users = t::run("who", &[p], "");
    assert_eq!(users.out().lines().count(), 1);
    assert!(users.out().contains("alice"));
    for (opt, needle) in [
        ("-b", "system boot"),
        ("-d", "dead"),
        ("-l", "LOGIN"),
        ("-p", "init"),
        ("-r", "run-level 5"),
        ("-t", "clock change"),
    ] {
        let r = t::run("who", &[opt, p], "");
        assert_eq!(r.code, 0, "opt {:?}", opt);
        assert!(r.out().contains(needle), "opt {:?} output {:?}", opt, r.out());
    }
    let all = t::run("who", &["-a", p], "");
    assert_eq!(all.out().lines().count(), 8);
    assert!(all.out().contains("term=0 exit=0"));
}

#[test]
fn count_mode_prints_names_then_total() {
    let f = file(
        "count",
        &[
            rec(7, 1, "pts/0", "p0", "alice", "", T),
            rec(7, 2, "pts/1", "p1", "bob", "", T),
            rec(6, 3, "tty1", "t1", "LOGIN", "", T),
        ],
    );
    let r = t::run("who", &["-q", f.to_str().unwrap()], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "alice bob\n# users=2\n");
    let r = t::run("who", &["--count", f.to_str().unwrap()], "");
    assert_eq!(r.out(), "alice bob\n# users=2\n");
    let empty = file("count0", &[]);
    let r = t::run("who", &["-q", empty.to_str().unwrap()], "");
    assert_eq!(r.out(), "\n# users=0\n");
}

#[test]
fn users_mode_adds_idle_and_pid_columns() {
    let f = file("users", &[rec(7, 4242, "pts/0", "p0", "alice", "h1", T)]);
    let r = t::run("who", &["-u", f.to_str().unwrap()], "");
    assert_eq!(r.code, 0);
    let out = r.out();
    let line = out.trim_end();
    assert!(line.starts_with(&format!("alice    pts/0        {} ", ts(T))), "line {:?}", line);
    assert!(line.ends_with("4242 h1"), "line {:?}", line);
    let idle = line.split("  ").collect::<Vec<&str>>().join("  ");
    assert!(idle.contains("4242"));
}

#[test]
fn dead_processes_show_as_gone_with_exit_status() {
    let f = file("dead", &[rec_full(8, 77, "pts/3", "p3", "old", "", T, 9, 1)]);
    let r = t::run("who", &["-a", f.to_str().unwrap()], "");
    assert_eq!(r.code, 0);
    let out = r.out();
    let line = out.trim_end();
    // -a includes the message-status column right after the name field.
    assert!(line.starts_with(&format!("old      ? pts/3        {} ", ts(T))), "line {:?}", line);
    assert!(line.contains("gone"), "line {:?}", line);
    assert!(line.contains(" 77 "), "line {:?}", line);
    assert!(line.ends_with("term=9 exit=1"), "line {:?}", line);
}

#[test]
fn missing_file_is_not_an_error() {
    let r = t::run("who", &["/nonexistent-utmp-file"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "");
    assert_eq!(r.err(), "");
    let r = t::run("who", &["--", "-x"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "");
}

#[test]
fn duplicate_options_are_rejected() {
    for (opt, long) in [("-s", "--short"), ("-a", "--all"), ("-u", "--users"), ("-H", "--heading")] {
        let r = t::run("who", &[opt, opt], "");
        assert_eq!(r.code, 1, "opt {:?}", opt);
        assert_eq!(
            r.err().lines().next().unwrap(),
            format!("error: the argument '{}' cannot be used multiple times", long)
        );
    }
    let r = t::run("who", &["--lookup", "--lookup"], "");
    assert_eq!(r.code, 1);
}

#[test]
fn unknown_option_is_a_clap_error_with_tip() {
    let r = t::run("who", &["--bogus"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: unexpected argument '--bogus' found\n\n  tip: to pass '--bogus' as a value, use '-- --bogus'\n\nUsage: who [OPTION]... [ FILE | ARG1 ARG2 ]\n\nFor more information, try '--help'.\n"
    );
    let r = t::run("who", &["-x"], "");
    assert_eq!(
        r.err(),
        "error: unexpected argument '-x' found\n\n  tip: to pass '-x' as a value, use '-- -x'\n\nUsage: who [OPTION]... [ FILE | ARG1 ARG2 ]\n\nFor more information, try '--help'.\n"
    );
}

#[test]
fn help_and_version() {
    let h = t::run("who", &["--help"], "").out();
    assert!(h.starts_with("Print information about the users currently on the machine, and their processes.\n"));
    assert!(h.contains("  -a, --all         same as -b -d --login -p -r -t -T -u\n"));
    assert!(h.contains("  -T, -W, --mesg    add user's message status as +, - or ?\n"));
    assert_eq!(t::run("who", &["--version"], "").out(), "who (uutils coreutils) 0.10.0\n");
}

#[test]
fn run_bin_dispatch() {
    let f = file("bin", &[rec(7, 5, "pts/0", "p0", "carol", "", T)]);
    let out = t::run_bin("who", &[f.to_str().unwrap()], "");
    assert_eq!(out.code, 0);
    assert!(out.out().contains("carol"));
}

#[test]
fn fuzz_invariants() {
    // 不變量：who FILE 印全部 USER_PROCESS(7) 记录、排除 DEAD(8)；-d 只印 DEAD；
    // -q：單行空格分隔 user 名稱（我方 E3 決定）；列數恆 1；旗標組合（-m/-s/-H 以外）不改變列數；rc ∈ {0,1,2}。
    let mut st: u64 = 0x9E37_7B01;
    for iter in 0..24u32 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let n = ((st >> 40) % 6 + 1) as usize;
        let mut recs: Vec<Vec<u8>> = Vec::new();
        let (mut live, mut dead) = (0usize, 0usize);
        for k in 0..n {
            let utype = [7u16, 7, 8, 0, 1][(st >> (k * 2)) as usize % 5];
            if utype == 7 {
                live += 1;
            } else if utype == 8 {
                dead += 1;
            }
            recs.push(rec_full(
                utype as i16,
                200 + k as i32 + iter as i32 * 10,
                &format!("pts/{k}"),
                &format!("p{k}"),
                &format!("u{}", k % 3),
                &format!("h{k}"),
                T + k as i32,
                0,
                0,
            ));
        }
        let f = file(&format!("fz{iter}"), &recs);
        let arg = f.to_str().unwrap();
        let r = t::run("who", &[arg], "");
        assert_eq!(r.code, 0, "iter {iter} {}", r.err());
        assert_eq!(r.out().lines().count(), live, "iter {iter} live={live} out={:?}", r.out());
        let d = t::run("who", &["-d", arg], "");
        assert_eq!(d.code, 0);
        assert_eq!(d.out().lines().count(), dead, "iter {iter} -d dead={dead}");
        let q = t::run("who", &["-q", arg], "");
        assert_eq!(q.code, 0);
        let __q = q.out();
        let mut name_tokens = 0usize;
        let mut seen_count = None;
        for line in __q.lines() {
            if let Some(rest) = line.strip_prefix("# users=") {
                seen_count = Some(rest.parse::<usize>().expect("users count"));
            } else {
                name_tokens += line.split_whitespace().count();
            }
        }
        assert_eq!(seen_count, Some(live), "iter {iter} -q {:?}", q.out());
        assert_eq!(name_tokens, live, "iter {iter} -q names {:?}", q.out());
        for opt in ["-s"] { // 只有 -s（不改列數）可作不變量；-l/-u/-m/-r 是 ut_type 過濾（另測於 heading）
            let m = t::run("who", &[opt, arg], "");
            assert_eq!(m.code, 0);
            assert_eq!(m.out().lines().count(), live, "iter {iter} {opt}");
        }
    }
    assert_eq!(t::run("who", &["--nope-who"], "").code, 1); // who 家族 clap rc = 1（見 assumptions E3）
}
