//! pinky(1) 測試（guest 內執行）。
//!
//! 我方行為基準（docs/man/pinky.1.gz + docs/assumptions.md「pinky」節）：
//! - 短格式欄位 USER(9)/TTY(9)/IDLE(6)/TIME(17)，行尾不留空白。
//! - -w 顯示登入名；-i 省略 USER 與 LOGIN FROM；-q 再省略 IDLE。
//! - -l 長格式：Login name / In real life / Directory / Shell / On since / Project / Plan；
//!   -b/-h/-p 分别省略 Directory+Shell / Project / Plan；多使用者以空行分隔。
//! - 未知使用者：`pinky: unknown user '<operand>'`（rc 1）。
//! - `-h` 是省略 project（非 help）；help 只有 `--help`。
//!
//! 測試直接改寫 guest 的 /var/run/utmp 與 /etc/passwd（guest 為 root、initramfs tmpfs 可寫），
//! 手法沿用 tests/id.rs 與 tests/who.rs。

use coreutils::sys;
use coreutils::testutil as t;
use coreutils::utmp;
use std::path::PathBuf;

const UTEST_USER: &str = "pinkyu";
const UTEST_HOME: &str = "/tmp/pinky_home";

fn run(args: &[&str]) -> t::Out {
    t::run("pinky", args, "")
}

fn rec(utype: i16, pid: i32, line: &str, id: &str, user: &str, host: &str, tv_sec: i32) -> Vec<u8> {
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
    b[340..344].copy_from_slice(&tv_sec.to_le_bytes());
    b
}

/// 佈席 guest 狀態：utmp 兩筆（一筆近期、一筆 old）+ 自訂 /etc/passwd。
/// 回傳 (tv_now, tv_old) 讓測試能算出預期的時間欄。
fn setup(tag: &str, with_files: bool) -> (i32, i32) {
    let now = sys::now_epoch() as i32;
    let old = now - 90000; // > 86400 → idle "old"
    std::fs::create_dir_all("/var/run").ok();
    std::fs::create_dir_all("/etc").ok();
    let mut data = Vec::new();
    data.extend_from_slice(&rec(utmp::USER_PROCESS, 100, "tty1", "1", UTEST_USER, "host.example", now));
    data.extend_from_slice(&rec(utmp::USER_PROCESS, 101, "pts/2", "2", "other", "", old));
    // 非 USER_PROCESS 應被忽略
    data.extend_from_slice(&rec(utmp::DEAD_PROCESS, 102, "pts/3", "3", "ghost", "", old));
    std::fs::write(utmp::DEFAULT_PATH, &data).expect("write utmp");

    std::fs::write(
        "/etc/passwd",
        format!("{UTEST_USER}:x:4001:4001:Pinky Tester:{UTEST_HOME}:/bin/sh\nother:x:4002:4002:Other User:/home/other:/bin/sh\nroot:x:0:0:root:/root,/bin/sh\n"),
    )
    .expect("write passwd");

    let home = PathBuf::from(UTEST_HOME);
    let _ = std::fs::remove_dir_all(&home);
    std::fs::create_dir_all(&home).expect("home");
    if with_files {
        std::fs::write(home.join(".project"), format!("project for {tag}\n")).expect("project");
        std::fs::write(home.join(".plan"), "plan line\n").expect("plan");
    }
    (now, old)
}

fn fmt_time(secs: i32) -> String {
    let (y, mo, d, h, mi) = sys::local_time(secs as i64).unwrap();
    format!("{:04}-{:02}-{:02} {:02}:{:02}", y, mo, d, h, mi)
}

fn pad(s: &str, w: usize) -> String {
    let n = s.chars().count();
    if n >= w {
        s.to_string()
    } else {
        format!("{}{}", s, " ".repeat(w - n))
    }
}

#[test]
fn help_version_and_dash_h() {
    let h = run(&["--help"]);
    assert_eq!(h.code, 0);
    assert!(h.out().starts_with("Displays brief user information"));
    assert!(h.out().contains("  -l               produce long format output"));
    assert_eq!(run(&["--version"]).out(), "pinky (uutils coreutils) 0.10.0\n");
    assert_eq!(run(&["-V"]).out(), "pinky (uutils coreutils) 0.10.0\n");
    // -h 不是 help：我方決定 -h 只修長格式（省略 project）；無 -l 時仍是短格式
    setup("dashh", true);
    let s = run(&["-h"]);
    assert_eq!(s.code, 0, "{}", s.err());
    assert!(!s.out().contains("Login name: "));
    assert!(s.out().contains("LOGIN FROM"));
    let l = run(&["-lh"]);
    assert_eq!(l.code, 0, "{}", l.err());
    assert!(l.out().contains("Login name: "));
    assert!(!l.out().contains("Usage: pinky"));
    assert!(!l.out().contains("Project:")); // -h 省略 project
}

#[test]
fn short_format_default() {
    let (now, old) = setup("short", false);
    let r = run(&[]);
    assert_eq!(r.code, 0);
    assert_eq!(r.err(), "");
    let __o = r.out();
    let lines: Vec<&str> = __o.lines().collect();
    assert_eq!(lines.len(), 3, "{:?}", r.out()); // header + 2 records
    assert_eq!(lines[0], "USER     TTY      IDLE  TIME             LOGIN FROM");
    assert_eq!(
        lines[1],
        format!(
            "{}{}{}{}host.example",
            pad("Pinky Te", 9),
            pad("tty1", 9),
            pad(".", 6),
            pad(&fmt_time(now), 17)
        )
    );
    assert_eq!(
        lines[2],
        format!("{}{}{}{}", pad("Other Us", 9), pad("pts/2", 9), pad("old", 6), pad(&fmt_time(old), 17)).trim_end()
    );
}

#[test]
fn short_omit_options_progressively_drop_columns() {
    let (now, _old) = setup("omit", false);
    let base = run(&[]).out();
    // -w：USER 改登入名
    let w = run(&["-w"]).out();
    assert!(w.lines().nth(1).unwrap().starts_with(&pad(UTEST_USER, 9)), "{w}");
    // -i：沒有 USER/LOGIN FROM
    let i = run(&["-i"]);
    assert_eq!(i.code, 0);
    assert!(!i.out().contains("LOGIN FROM"));
    assert!(!i.out().contains("host.example"));
    assert!(!i.out().contains("Pinky Tester"));
    // -q：再沒有 IDLE
    let q = run(&["-q"]);
    assert_eq!(q.code, 0);
    assert!(!q.out().contains("IDLE"));
    assert!(!q.out().contains("host.example"));
    // TIME 欄仍在
    assert!(q.out().contains(&fmt_time(now)));
    // -f：去掉標頭行，列數不變
    let f = run(&["-f"]);
    let base_lines: Vec<&str> = base.lines().collect();
    let __o = f.out();
    let f_lines: Vec<&str> = __o.lines().collect();
    assert_eq!(f_lines.len(), base_lines.len() - 1);
    assert_eq!(f_lines, &base_lines[1..]);
}

#[test]
fn short_filters_by_operand() {
    setup("filter", false);
    let r = run(&[UTEST_USER]);
    assert_eq!(r.code, 0);
    let __o = r.out();
    let lines: Vec<&str> = __o.lines().collect();
    assert_eq!(lines.len(), 2, "{:?}", r.out());
    assert!(lines[1].contains("host.example"));
    // 未知使用者
    let bad = run(&["nosuchuser"]);
    assert_eq!(bad.code, 1);
    assert_eq!(bad.err(), "pinky: unknown user 'nosuchuser'\n");
    // 混合：已知 + 未知 → 仍有輸出且 rc 1
    let mix = run(&[UTEST_USER, "nosuchuser"]);
    assert_eq!(mix.code, 1);
    assert!(mix.out().contains("host.example"));
}

#[test]
fn long_format_blocks() {
    let (now, _old) = setup("long", true);
    let r = run(&["-l", UTEST_USER]);
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    assert!(out.contains("Login name: pinkyu\n"));
    assert!(out.contains("In real life: Pinky Tester\n"));
    assert!(out.contains("Directory: /tmp/pinky_home\n"));
    assert!(out.contains("Shell: /bin/sh\n"));
    assert!(out.contains(&format!("On since {} (UTC) on tty1 (host.example)\n", fmt_time(now))));
    assert!(out.contains("Project:\nproject for long\n"));
    assert!(out.contains("Plan:\nplan line\n"));

    // -b 省略 Directory/Shell；-h 省略 Project；-p 省略 Plan
    let b = run(&["-lb", UTEST_USER]).out();
    assert!(!b.contains("Directory:"));
    assert!(!b.contains("Shell:"));
    let hh = run(&["-lh", UTEST_USER]).out();
    assert!(!hh.contains("Project:"));
    assert!(hh.contains("Plan:"));
    let pp = run(&["-lp", UTEST_USER]).out();
    assert!(pp.contains("Project:"));
    assert!(!pp.contains("Plan:"));
    // -s 蓋掉 -l（短格式）
    let s = run(&["-ls", UTEST_USER]).out();
    assert!(!s.contains("Login name:"));
    assert!(s.contains("LOGIN FROM"));
}

#[test]
fn long_multiple_users_separated_by_blank_line() {
    setup("multi", false);
    let r = run(&["-l", UTEST_USER, "other"]);
    assert_eq!(r.code, 0, "{}", r.err());
    let __o = r.out();
    let blocks: Vec<&str> = __o.split("\n\n").collect();
    assert_eq!(blocks.len(), 2, "{:?}", r.out());
    assert!(blocks[0].starts_with("Login name: pinkyu"));
    assert!(blocks[1].starts_with("Login name: other"));
    // 無 operand 的 -l：全部 utmp 使用者
    let all = run(&["-l"]);
    assert!(all.out().contains("Login name: ghost") == false); // DEAD_PROCESS 不出現
    let __o = all.out();
    let all_blocks: Vec<&str> = __o.split("\n\n").collect();
    assert_eq!(all_blocks.len(), 2, "{:?}", all.out());
}

#[test]
fn clap_style_errors() {
    let r = run(&["--bogus"]);
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("error: unexpected argument '--bogus' found\n\n"), "{}", r.err());
    assert!(r.err().contains("Usage: pinky [OPTION]... [USER]..."));

    let r = run(&["-x"]);
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("error: unexpected argument '-x' found\n\n"));

    // 重複旗標
    for pair in [vec!["-l", "-l"], vec!["-l", "--help=x"], vec!["--lookup", "--lookup"]] {
        let r = run(&pair);
        if pair == vec!["-l", "--help=x"] {
            assert_eq!(r.code, 1, "{:?}", pair);
            assert!(r.err().starts_with("error: unexpected value 'x' for '--help' found"), "{}", r.err());
            continue;
        }
        assert_eq!(r.code, 2, "{:?}", pair);
        let expect = if pair[0] == "-l" {
            "error: the argument '-l' cannot be used multiple times\n\nFor more information, try '--help'.\n"
        } else {
            "error: the argument '--lookup' cannot be used multiple times\n\nFor more information, try '--help'.\n"
        };
        assert_eq!(r.err(), expect, "{:?}", pair);
    }

    // 前綴解析：--loo → lookup；--lon 沒有候選 → unexpected
    setup("prefix", false);
    let r = run(&["--loo"]);
    assert_eq!(r.code, 0, "{}", r.err());
    let r = run(&["--lon"]);
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("error: unexpected argument '--lon' found\n\n"), "{}", r.err());

    // -- 之後一律 positional
    let r = run(&["--", "-l"]);
    assert_eq!(r.code, 1); // 「-l」是不存在的使用者
    assert_eq!(r.err(), "pinky: unknown user '-l'\n");
}

#[test]
fn fuzz_invariants() {
    setup("fuzz", false);
    let base = run(&[]).out();
    let base_lines = base.lines().count();
    // 不變量 1：選項只增不減列數（-f 恰好少一個標頭）
    for opt in ["-w", "-i", "-q", "-s", "-b", "-p"] {
        let n = run(&[opt]).out().lines().count();
        assert_eq!(n, base_lines, "opt {opt}");
    }
    assert_eq!(run(&["-f"]).out().lines().count(), base_lines - 1);
    // 不變量 2：任意選項組合不 panic、exit code ∈ {0,1,2}
    let pool = ["-l", "-b", "-h", "-p", "-s", "-f", "-w", "-i", "-q", "--lookup", "--", UTEST_USER, "other"];
    let seed: u64 = 0x5EED;
    let mut st = seed;
    for iter in 0..60u64 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let n = (st >> 40) % 5 + 1;
        let mut args: Vec<&str> = Vec::new();
        for j in 0..n {
            args.push(pool[((st >> (j * 4)) % pool.len() as u64) as usize]);
        }
        let r = run(&args);
        assert!(r.code == 0 || r.code == 1 || r.code == 2, "iter {} {:?} -> {}", iter, args, r.code);
    }
    // 不變量 3：短格式每一列都含 TTY 欄（tty1 / pts/2）
    for line in base.lines().skip(1) {
        assert!(line.contains("tty1") || line.contains("pts/2"), "{line}");
    }
}
