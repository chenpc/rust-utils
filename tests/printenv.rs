//! printenv(1) guest e2e 測試。
//!
//! 斷言依據：docs/man/printenv.1.gz（`-0, --null  end each output line with 0 byte rather
//! than newline`、operand 為 VARIABLE 清單）＋ man 未載明時由本機參考實測決定的部分
//! （無 operand 依 environ 原始順序、缺任一 VARIABLE 即 rc1 且不寫 stderr、
//! 選項錯誤的 clap 措辭與 rc2），見 docs/assumptions.md。

use coreutils::testutil as t;

fn setv(name: &str, value: &str) {
    std::env::set_var(name, value);
}

#[test]
fn no_operands_prints_all_pairs() {
    setv("PE_ALL_A", "va");
    setv("PE_ALL_B", "vb");
    let r = t::run("printenv", &[], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.err(), "");
    let out = r.out();
    let mut saw_a = false;
    let mut saw_b = false;
    for line in out.lines() {
        assert!(line.contains('='), "每一行都必須是 NAME=VALUE：{}", line);
        if line == "PE_ALL_A=va" {
            saw_a = true;
        }
        if line == "PE_ALL_B=vb" {
            saw_b = true;
        }
    }
    assert!(saw_a && saw_b, "{}", out);
    // 輸出以換行收尾
    assert!(out.ends_with('\n'), "{:?}", out);
}

#[test]
fn single_var_prints_value_only() {
    setv("PE_ONE", "the value");
    let r = t::run("printenv", &["PE_ONE"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "the value\n");
    assert_eq!(r.err(), "");
}

#[test]
fn empty_value_var_prints_empty_line() {
    setv("PE_EMPTY", "");
    let r = t::run("printenv", &["PE_EMPTY"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "\n");
}

#[test]
fn missing_var_is_rc1_with_silent_stderr() {
    let r = t::run("printenv", &["PE_DEFINITELY_NOT_SET"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.out(), "");
    assert_eq!(r.err(), "");
    // 空字串名稱同樣算不存在
    let r2 = t::run("printenv", &[""], "");
    assert_eq!(r2.code, 1);
    assert_eq!(r2.out(), "");
    assert_eq!(r2.err(), "");
}

#[test]
fn mix_of_existing_and_missing_prints_then_rc1() {
    setv("PE_MIX1", "m1");
    setv("PE_MIX2", "m2");
    let r = t::run(
        "printenv",
        &["PE_MIX1", "PE_NOT_SET", "PE_MIX2"],
        "",
    );
    assert_eq!(r.code, 1);
    assert_eq!(r.out(), "m1\nm2\n");
    assert_eq!(r.err(), "");
    // 全部存在 → rc0
    let r2 = t::run("printenv", &["PE_MIX1", "PE_MIX2"], "");
    assert_eq!(r2.code, 0);
    assert_eq!(r2.out(), "m1\nm2\n");
    // 重複同一變數會印兩次
    let r3 = t::run("printenv", &["PE_MIX1", "PE_MIX1"], "");
    assert_eq!(r3.code, 0);
    assert_eq!(r3.out(), "m1\nm1\n");
}

#[test]
fn null_flag_ends_lines_with_nul() {
    setv("PE_NUL", "n1");
    let r = t::run("printenv", &["-0", "PE_NUL"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out_bytes(), b"n1\x00");
    let r2 = t::run("printenv", &["--null", "PE_NUL", "PE_NUL"], "");
    assert_eq!(r2.out_bytes(), b"n1\x00n1\x00");
    // 無 operand 時也用 NUL 分隔
    let r3 = t::run("printenv", &["-0"], "");
    assert_eq!(r3.code, 0);
    let bytes = r3.out_bytes();
    assert!(bytes.ends_with(b"\x00"), "{:?}", bytes);
    assert!(!bytes.contains(&b'\n'), "{:?}", bytes);
    let lossy = String::from_utf8_lossy(&bytes).to_string();
    assert!(lossy.contains("PE_NUL=n1\u{0}"), "{:?}", bytes);
}

#[test]
fn dup_null_is_clap_error_rc2() {
    for args in [
        &["-0", "-0"][..],
        &["-00"][..],
        &["--null", "--null"][..],
        &["--nul", "--null"][..],
        &["-0", "--null"][..],
    ] {
        let r = t::run("printenv", args, "");
        assert_eq!(r.code, 2, "{:?}", args);
        assert_eq!(
            r.err(),
            "error: the argument '--null' cannot be used multiple times\n\nFor more information, try '--help'.\n",
            "{:?}",
            args
        );
        assert_eq!(r.out(), "");
    }
}

#[test]
fn unknown_option_is_clap_style_rc2() {
    for args in [&["-x"][..], &["--bogus"][..], &["-i"][..], &["-1"][..], &["--nulx"][..]] {
        let r = t::run("printenv", args, "");
        assert_eq!(r.code, 2, "{:?}", args);
        let err = r.err();
        let lines: Vec<&str> = err.lines().collect();
        assert!(lines[0].starts_with("error: unexpected argument '"), "{}", err);
        assert_eq!(lines[1], "");
        assert!(err.contains("  tip: to pass '"), "{}", err);
        assert!(
            err.contains("Usage: printenv [OPTION]... [VARIABLE]..."),
            "{}",
            err
        );
        assert!(err.contains("For more information, try '--help'."), "{}", err);
    }
}

#[test]
fn double_dash_turns_flags_into_operands() {
    setv("PE_DD", "dd");
    // `--` 之後的 `-x` 變成變數名稱（不存在 → rc1、無輸出）
    let r = t::run("printenv", &["--", "-x"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.out(), "");
    // 單一 `-` 也是 operand（不是選項）
    let r2 = t::run("printenv", &["-"], "");
    assert_eq!(r2.code, 1);
    assert_eq!(r2.out(), "");
    // 同一變數在 `--` 前後各印一次
    let r3 = t::run("printenv", &["PE_DD", "--", "PE_DD"], "");
    assert_eq!(r3.code, 0);
    assert_eq!(r3.out(), "dd\ndd\n");
}

#[test]
fn help_and_version_are_verbatim() {
    let r = t::run("printenv", &["--help"], "");
    assert_eq!(r.code, 0);
    let expect = "Display the values of the specified environment VARIABLE(s), or (with no VARIABLE) display name and\n\
                  value pairs for them all.\n\
                  \n\
                  Usage: printenv [OPTION]... [VARIABLE]...\n\
                  \n\
                  Arguments:\n\
                  \x20 [variables]...  \n\
                  \n\
                  Options:\n\
                  \x20 -0, --null     end each output line with 0 byte rather than newline\n\
                  \x20 -h, --help     Print help\n\
                  \x20 -V, --version  Print version\n";
    assert_eq!(r.out(), expect);
    assert_eq!(t::run("printenv", &["-h"], "").out(), expect);
    assert_eq!(t::run("printenv", &["--hel"], "").out(), expect);
    let v = t::run("printenv", &["--version"], "");
    assert_eq!(v.out(), "printenv (uutils coreutils) 0.10.0\n");
    assert_eq!(t::run("printenv", &["--vers"], "").out(), v.out());
    // help 的重复不會觸發 clap 錯誤
    let r = t::run("printenv", &["--help", "--help"], "");
    assert_eq!(r.out(), expect);
    // help/version 先出現就立刻回 0（不處理後續 operand）
    setv("PE_AFTER", "after");
    assert_eq!(t::run("printenv", &["--help", "PE_AFTER"], "").out(), expect);
    assert_eq!(t::run("printenv", &["PE_AFTER", "--version"], "").out(), v.out());
}

#[test]
fn run_bin_e2e() {
    setv("PE_BIN", "binval");
    let r = t::run_bin("printenv", &["PE_BIN"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "binval\n");
    let r = t::run_bin("printenv", &["-0", "PE_BIN"], "");
    assert_eq!(r.out_bytes(), b"binval\x00");
    let r = t::run_bin("printenv", &["PE_DEFINITELY_NOT_SET"], "");
    assert_eq!(r.code, 1);
}

#[test]
fn fuzz_names_and_values() {
    let mut rng = t::Rng::new(20261126);
    for round in 0..20 {
        let n = 1 + rng.below(4);
        let mut names: Vec<String> = Vec::new();
        let mut args: Vec<String> = Vec::new();
        let mut want: Vec<String> = Vec::new();
        for _k in 0..n {
            let len = 4 + rng.below(8);
            let alphabet: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ_0123456789";
            let name: String = (0..len)
                .map(|_| alphabet[rng.below(alphabet.len() as u64) as usize] as char)
                .collect();
            let name = format!("FUZZ_{}", name);
            let value: String = if rng.below(4) == 0 {
                String::new()
            } else {
                (0..6).map(|_| (b'x' + rng.below(20) as u8) as char).collect()
            };
            let present = rng.below(3) != 0;
            if present {
                setv(&name, &value);
                want.push(value.clone());
            }
            names.push(name.clone());
            args.push(name);
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let r = t::run("printenv", &refs, "");
        let out = r.out();
        let got: Vec<&str> = out.lines().collect();
        assert_eq!(got, want.iter().map(|s| s.as_str()).collect::<Vec<_>>(), "round {}", round);
        // 任一變數不存在 → rc1；全部存在 → rc0
        let all_present = got.len() == args.len();
        assert_eq!(r.code, if all_present { 0 } else { 1 }, "round {}", round);
        assert_eq!(r.err(), "");
    }
}
