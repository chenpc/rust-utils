//! tty(1)：「Check if stdin is a terminal.」
//!
//! 有兩條 stdin 路徑要分開測：
//! - `run_bin` 的 stdin 一定是 pipe → 必然 `not a tty`（rc 1）。
//! - in-process `run` 的 fd 0 是 test binary 自己的 stdin（guest 內可能是序列主控台），
//!   所以該測試對兩種結果都接受，只要求「有/無輸出」與 `-s` 一致。

use coreutils::testutil as t;

fn run(args: &[&str]) -> t::Out {
    t::run("tty", args, "")
}

#[test]
fn version_and_help() {
    let r = run(&["--version"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "tty (uutils coreutils) 0.10.0\n");
    assert_eq!(r.err(), "");

    let h = run(&["--help"]);
    assert_eq!(h.code, 0);
    let out = h.out();
    // help 文字是我方從 man OPTIONS 推導的版本（非參考逐字）。
    assert_eq!(
        out,
        "Print the file name of the terminal connected to standard input.\n\nUsage: tty [OPTION]...\n\nOptions:\n  -s, --silent   print nothing, only return an exit status [alias: --quiet]\n  -h, --help     Print help\n  -V, --version  Print version\n"
    );
    assert_eq!(run(&["-h"]).out(), out);
}

#[test]
fn pipe_stdin_is_not_a_tty() {
    let r = t::run_bin("tty", &[], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.out(), "not a tty\n");
    assert_eq!(r.err(), "");

    // 我方 short 集只列 man 的 -s/-h/-V；--quiet 是長形 alias（無 -q/-S 短形）。
    for flag in ["-s", "--silent", "--quiet"] {
        let s = t::run_bin("tty", &[flag], "");
        assert_eq!(s.code, 1, "flag {}", flag);
        assert_eq!(s.out(), "", "flag {}", flag);
        assert_eq!(s.err(), "", "flag {}", flag);
    }
}

#[test]
fn own_stdin_is_consistent_with_silent() {
    let a = run(&[]);
    let b = run(&["-s"]);
    assert_eq!(b.err(), "");
    assert_eq!(b.out(), "");
    if a.code == 0 {
        let name = a.out();
        // 是 tty 時不附換行（man 只說印檔名；參考實測如此）。
        assert!(!name.ends_with('\n'), "out {:?}", name);
        assert!(name.starts_with("/dev/"), "out {:?}", name);
        assert_eq!(b.code, 0);
    } else {
        assert_eq!(a.code, 1);
        assert_eq!(a.out(), "not a tty\n");
        assert_eq!(b.code, 1);
    }
}

#[test]
fn positional_argument_is_unexpected() {
    for arg in ["x", "y"] {
        let r = run(&[arg]);
        assert_eq!(r.code, 2);
        assert_eq!(
            r.err(),
            format!("error: unexpected argument '{}' found\n\nUsage: tty [OPTION]...\n\nFor more information, try '--help'.\n", arg)
        );
        assert_eq!(r.out(), "");
    }
    // `--` 之後的 token 一律當 positional（ tty 沒有 positional，所以直接報錯）。
    let r = run(&["--", "--help"]);
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("error: unexpected argument '--help' found"));
}

#[test]
fn unknown_short_flags_report_the_offending_char() {
    // clap 逐字元報；字元 `0` 顯示成 `0`（不帶 dash）。
    let cases: &[(&str, &str)] = &[
        ("-x", "'-x'"),
        ("-a", "'-a'"),
        ("-!", "'-!'"),
        (".", "'.'"),
        ("-1", "'-1'"),
        ("-9", "'-9'"),
        ("-3x", "'-3'"),
        ("-x0", "'-x'"),
        ("-0", "'0'"),
        ("-00", "'0'"),
        ("-0x", "'0'"),
        ("-s0", "'0'"),
        ("-sq", "'-q'"),
        ("-qs", "'-q'"),
        ("-S", "'-S'"),
    ];
    for (arg, shown) in cases {
        let r = run(&[arg]);
        assert_eq!(r.code, 2, "arg {}", arg);
        let expected = format!(
            "error: unexpected argument {} found\n\nUsage: tty [OPTION]...\n\nFor more information, try '--help'.\n",
            shown
        );
        assert_eq!(r.err(), expected, "arg {}", arg);
    }
}

#[test]
fn unknown_long_flags() {
    let r = run(&["--foo"]);
    assert_eq!(r.code, 2);
    assert_eq!(
        r.err(),
        "error: unexpected argument '--foo' found\n\nUsage: tty [OPTION]...\n\nFor more information, try '--help'.\n"
    );
    // 唯一前綴可解析。
    assert_eq!(run(&["--s"]).code, 1);
    assert_eq!(run(&["--si"]).code, 1);
    assert_eq!(run(&["--quiet"]).code, 1);
    assert_eq!(run(&["--ver"]).out(), "tty (uutils coreutils) 0.10.0\n");
    assert_eq!(run(&["--hel"]).code, 0);
}

#[test]
fn flag_with_attached_value_is_rejected() {
    let cases: &[(&str, &str, &str)] = &[
        ("--silent=x", "silent", "x"),
        ("--quiet=x", "silent", "x"),
        ("--help=x", "help", "x"),
        ("--version=x", "version", "x"),
        ("--s=x", "silent", "x"),
    ];
    for (arg, long, value) in cases {
        let r = run(&[arg]);
        assert_eq!(r.code, 2, "arg {}", arg);
        assert_eq!(
            r.err(),
            format!(
                "error: unexpected value '{}' for '--{}' found; no more were expected\n\nUsage: tty [OPTION]...\n\nFor more information, try '--help'.\n",
                value, long
            ),
            "arg {}",
            arg
        );
    }
}

#[test]
fn duplicate_silent_is_rejected() {
    for pair in [
        vec!["-s", "-s"],
        vec!["--silent", "--silent"],
        vec!["--quiet", "--quiet"],
        vec!["--silent", "-s"],
        vec!["--quiet", "-s"],
        vec!["-s", "--quiet"],
    ] {
        let args: Vec<&str> = pair.iter().copied().collect();
        let r = run(&args);
        assert_eq!(r.code, 2, "args {:?}", args);
        assert_eq!(
            r.err(),
            "error: the argument '--silent' cannot be used multiple times\n\nFor more information, try '--help'.\n",
            "args {:?}",
            args
        );
    }
}

#[test]
fn short_clusters_still_work() {
    // -hs / -sh：h 先結束；-sV、-V0 同理。
    let r = run(&["-hs"]);
    assert_eq!(r.code, 0);
    assert!(r.out().starts_with("Print the file name"));
    let r = run(&["-V0"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "tty (uutils coreutils) 0.10.0\n");
}

#[test]
fn fuzz_invariants() {
    // 不變量：任意選項組合 rc ∈ {0,1,2}；rc==2 時 stdout 恆空；
    // silent 族（-s/--silent/--quiet 與其唯一前綴）→ rc 0 且零輸出；
    // rc 1 → 恰 "not a tty\n"。
    let pool = ["-s", "--silent", "--quiet", "--qui", "-h", "-V", "--help", "--version"];
    let mut st: u64 = 0x71A5E;
    for _ in 0..80 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let n = (st >> 50) % 4 + 1;
        let mut args: Vec<&str> = Vec::new();
        for k in 0..n {
            args.push(pool[((st >> (k * 6)) % pool.len() as u64) as usize]);
        }
        let r = run(&args);
        assert!(matches!(r.code, 0 | 1 | 2), "{:?} -> {}", args, r.code);
        if r.code == 2 {
            assert_eq!(r.out(), "", "{:?}", args);
        }
        let silent = args
            .iter()
            .filter(|a| matches!(**a, "-s" | "--silent" | "--quiet" | "--qui"))
            .count();
        let info = args.iter().any(|a| matches!(*a, "-h" | "-V" | "--help" | "--version"));
        if silent >= 2 && !info {
            // silent 家族是同一選項的 alias：出現兩次 → dup（rc 2、零輸出）
            assert_eq!(r.code, 2, "{:?}", args);
            assert_eq!(r.out(), "", "{:?}", args);
            assert!(r.err().starts_with("error: the argument '--silent' cannot be used multiple times"), "{:?} {:?}", args, r.err());
        } else if silent == 1 && !info {
            // -s：exit status 就是 tty 檢查結果（stdin 是 pipe → 通常 1），恆零輸出
            assert!(r.code == 0 || r.code == 1, "{:?} -> {}", args, r.code);
            assert_eq!(r.out(), "", "{:?}", args);
        }
        if r.code == 1 && silent == 0 {
            assert_eq!(r.out(), "not a tty\n", "{:?}", args);
        }
        if info && silent == 0 {
            assert_eq!(r.code, 0, "{:?}", args);
        }
    }
}
