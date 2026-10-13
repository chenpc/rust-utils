//! factor(1)：`factor [NUMBER]...`（無 operand 時讀 stdin）。
//!
//! 對齊基準：本機參考 `/usr/lib/cargo/bin/coreutils/factor`（uutils coreutils 0.10.0）
//! 的 pair-diff 探測；CLI 錯誤採 clap 措辭（factor 的 clap rc = **1**）。
//! 任意精度分解的實作細節見 `src/applets/factor.rs` 頂端註解與 docs/assumptions.md。

use coreutils::testutil as t;

fn run(args: &[&str]) -> t::Out {
    t::run("factor", args, "")
}

const USAGE: &str = "Usage: factor [OPTION]... [NUMBER]...";

#[test]
fn small_numbers() {
    let r = run(&["2", "36", "1234567890"]);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out(),
        "2: 2\n36: 2 2 3 3\n1234567890: 2 3 3 5 3607 3803\n"
    );
    assert_eq!(r.err(), "");
}

#[test]
fn zero_and_one_have_empty_factor_list() {
    assert_eq!(run(&["0"]).out(), "0:\n");
    assert_eq!(run(&["1"]).out(), "1:\n");
    assert_eq!(run(&["1", "0", "2"]).out(), "1:\n0:\n2: 2\n");
}

#[test]
fn u64_boundary_and_known_semirimes() {
    assert_eq!(
        run(&["18446744073709551615"]).out(),
        "18446744073709551615: 3 5 17 257 641 65537 6700417\n"
    );
    assert_eq!(
        run(&["18446744073709551616"]).out(),
        String::from("18446744073709551616: ") + &"2 ".repeat(63) + "2\n"
    );
    assert_eq!(
        run(&["9223372036854775807"]).out(),
        "9223372036854775807: 7 7 73 127 337 92737 649657\n"
    );
    assert_eq!(
        run(&["34050042207616155167"]).out(),
        "34050042207616155167: 23 78349693 18895244653\n"
    );
    // 2^64 + 1 = 274177 × 67280421310721
    assert_eq!(
        run(&["274177", "67280421310721"]).out(),
        "274177: 274177\n67280421310721: 67280421310721\n"
    );
}

#[test]
fn arbitrary_precision_inputs() {
    // 大質數直接輸出自身
    assert_eq!(
        run(&["1000000000000000000000007"]).out(),
        "1000000000000000000000007: 1000000000000000000000007\n"
    );
    // 31 位：試除 2、5 之後 59 × 3188586931 × 12468617093648341
    assert_eq!(
        run(&["23456789012345678901234567890"]).out(),
        "23456789012345678901234567890: 2 5 59 3188586931 12468617093648341\n"
    );
    // 40 位（舊的逐位加倍 mul_mod 在這裡會 >25s）
    assert_eq!(
        run(&["1234567890123456789012345678901234567890"]).out(),
        "1234567890123456789012345678901234567890: 2 3 3 5 73 101 137 3541 3607 3803 27961 1676321 5964848081\n"
    );
    // 49 個 9
    assert_eq!(
        run(&["99999999999999999999999999999999999999999999999"]).out(),
        "99999999999999999999999999999999999999999999999: 3 3 35121409 316362908763458525001406154038726382279\n"
    );
    // 2^70 + 3 = 13 × 25087 × 3619992029943217
    assert_eq!(
        run(&["1180591620717411303427"]).out(),
        "1180591620717411303427: 13 25087 3619992029943217\n"
    );
    // 41 位半質數：rho 需要多輪 restart 才會撞出 18 位因子
    let r = run(&["1797693134862315907729305190789024733617976975931"]);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out(),
        "1797693134862315907729305190789024733617976975931: 3 71 3050978993 2766283836092290141393974186642380159\n"
    );
    assert_eq!(r.err(), "");
}

#[test]
fn exponents_flag() {
    assert_eq!(
        run(&["--exponents", "100", "360", "4096", "12"]).out(),
        "100: 2^2 5^2\n360: 2^3 3^2 5\n4096: 2^12\n12: 2^2 3\n"
    );
    // 短旗標是 -h（--help 的縮寫同時啟用指數格式），-e 不存在
    assert_eq!(run(&["-h", "12"]).out(), "12: 2^2 3\n");
    assert_eq!(run(&["--exponents", "--exponents", "4"]).out(), "4: 2^2\n");
}

#[test]
fn stdin_mode_when_no_operands() {
    let r = t::run("factor", &[], "5 6\n7\n\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "5: 5\n6: 2 3\n7: 7\n");
    assert_eq!(r.err(), "");
    // 空白分隔也走 stdin
    let r2 = t::run("factor", &["--"], "12\n");
    assert_eq!(r2.out(), "12: 2 2 3\n");
}

#[test]
fn invalid_tokens_report_and_continue() {
    let r = run(&["2", "abc", "3"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.out(), "2: 2\n3: 3\n");
    assert_eq!(r.err(), "factor: 'abc' is not a valid positive integer\n");

    for bad in &["0x10", "1e3", "1.5", "12a", "+abc", "", "1-2", "5_0"] {
        let rb = run(&[bad]);
        assert_eq!(rb.code, 1, "token {:?}", bad);
        assert_eq!(rb.out(), "");
        assert_eq!(
            rb.err(),
            format!("factor: '{}' is not a valid positive integer\n", bad)
        );
    }
}

#[test]
fn leading_plus_and_leading_zeros_are_accepted() {
    assert_eq!(run(&["+7"]).out(), "7: 7\n");
    assert_eq!(run(&["007"]).out(), "7: 7\n");
    assert_eq!(run(&["+0012"]).out(), "12: 2 2 3\n");
    assert_eq!(run(&["+0"]).out(), "0:\n");
}

#[test]
fn clap_style_errors() {
    let r = run(&["--bogus"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.out(), "");
    assert_eq!(
        r.err(),
        format!(
            "error: unexpected argument '--bogus' found\n\n  tip: to pass '--bogus' as a value, use '-- --bogus'\n\n{}\n\nFor more information, try '--help'.\n",
            USAGE
        )
    );

    // 前綴相符 → similar tip
    // CLI 錯誤措輞見 docs/assumptions.md
        let s = run(&["--bog"]);
    assert_eq!(s.code, 1);
    let want = String::from("error: unexpected argument '--bog' found\n\n")
        + "  tip: to pass '--bog' as a value, use '-- --bog'\n\n"
        + USAGE + "\n\nFor more information, try '--help'.\n";
    assert_eq!(s.err(), want);

    let cluster = run(&["-1x"]);
    assert_eq!(cluster.code, 1);
    assert!(cluster.err().contains("error: unexpected argument '-1' found"));

let x = run(&["-x"]);
    assert_eq!(x.code, 1);
    assert!(x.err().contains("error: unexpected argument '-x' found"));
    let one = run(&["-1"]);
    assert_eq!(one.code, 1);
    assert!(one.err().contains("error: unexpected argument '-1' found"));
    assert!(one.err().contains("  tip: to pass '-1' as a value, use '-- -1'"));
    // factor 的指數短旗標是 -h；-e 不存在
    let e = run(&["-e", "8"]);
    assert_eq!(e.code, 1);
    assert!(e.err().contains("error: unexpected argument '-e' found"));

    // 不需要值的旗標卻附了值
    for (long, val) in [
        ("exponents", "x"),
        ("help", "1"),
        ("version", "1"),
    ] {
        let v = run(&[&format!("--{}={}", long, val)]);
        assert_eq!(v.code, 1, "long {}", long);
        assert_eq!(v.out(), "");
        assert_eq!(
            v.err(),
            format!(
                "error: unexpected value '{}' for '--{}' found; no more were expected\n\n{}\n\nFor more information, try '--help'.\n",
                val, long, USAGE
            )
        );
    }

    // 唯一前綴可解析
    assert_eq!(run(&["--exp", "12"]).out(), "12: 2^2 3\n");
    assert_eq!(run(&["--ver"]).out(), "factor (uutils coreutils) 0.10.0\n");
    assert_eq!(run(&["--h"]).out(), run(&["--help"]).out());
}

#[test]
fn double_dash_and_operand_order_are_preserved() {
    let r = run(&["--", "--1"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "factor: '--1' is not a valid positive integer\n");

    assert_eq!(run(&["--", "10"]).out(), "10: 2 5\n");
    // 旗標可貫穿 positional
    assert_eq!(run(&["36", "-h"]).out(), "36: 2^2 3^2\n");
    // 輸出順序依輸入順序，因子本身由小到大
    assert_eq!(run(&["100", "4"]).out(), "100: 2 2 5 5\n4: 2 2\n");
    assert_eq!(run(&["20"]).out(), "20: 2 2 5\n");
}

#[test]
fn help_and_version_text() {
    let v = run(&["--version"]);
    assert_eq!(v.code, 0);
    assert_eq!(v.out(), "factor (uutils coreutils) 0.10.0\n");

    let h = run(&["--help"]);
    assert_eq!(h.code, 0);
    let out = h.out();
    assert!(
        out.starts_with("Print the prime factors of the given NUMBER(s).\n"),
        "{:?}",
        out
    );
    assert!(out.contains("If none are specified, read from standard input."));
    assert!(out.contains("Usage: factor [OPTION]... [NUMBER]..."));
    assert!(out.contains("  -h, --exponents"));
    assert!(out.contains("Print factors in the form p^e"));
    assert!(out.contains("  -V, --version"));
    // CLI 錯誤措輞見 docs/assumptions.md
    assert_eq!(run(&["-h", "12"]).out(), "12: 2^2 3\n");
}

#[test]
fn via_binary_spawns_stdin_and_exit_codes() {
    let r = t::run_bin("factor", &[], "5 6 7\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "5: 5\n6: 2 3\n7: 7\n");

    let b = t::run_bin("factor", &["abc"], "");
    assert_eq!(b.code, 1);
    assert_eq!(b.err(), "factor: 'abc' is not a valid positive integer\n");

    // 负數被當成旗標 → clap 錯誤（rc 1）
    let n = t::run_bin("factor", &["-1"], "");
    assert_eq!(n.code, 1);
    assert!(n.err().contains("error: unexpected argument '-1' found"));
}

#[test]
fn fuzz_invariants() {
    // 不變量 1：隨機合成數 n = p*q（小質數表）→ 輸出因子排好序、連乘積 == n、全部 >1。
    let primes: [u64; 16] = [2, 3, 5, 7, 11, 13, 17, 19, 23, 29, 31, 37, 41, 43, 47, 53];
    let mut st: u64 = 0xF4C7_0A5E;
    for _ in 0..120 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let p = primes[((st >> 30) % 16) as usize];
        let q = primes[((st >> 20) % 16) as usize];
        let r = primes[((st >> 10) % 16) as usize];
        let n = p * q * r;
        let out = run(&[&n.to_string()]);
        assert_eq!(out.code, 0);
        let line = out.out();
        let line = line.trim();
        let (lhs, rest) = line.split_once(": ").unwrap_or((line, ""));
        assert_eq!(lhs.parse::<u64>().unwrap(), n, "{line}");
        let fs: Vec<u64> = rest.split_whitespace().map(|s| s.parse().unwrap()).collect();
        assert!(!fs.is_empty() && fs.windows(2).all(|w| w[0] <= w[1]), "{line}");
        assert_eq!(fs.iter().product::<u64>(), n, "{line}");
        for f in &fs {
            assert!(*f > 1 && n % f == 0, "{line}");
        }
    }
    // 不變量 2：質數自身 → "p: p"；隨機垃圾 token → rc 1 且訊息固定。
    for p in [1000003u64, 9999999967, 2147483647] {
        let out = run(&[&p.to_string()]).out();
        assert_eq!(out.trim(), format!("{p}: {p}"));
    }
    let mut st2: u64 = 0xBADF00D1;
    for _ in 0..60 {
        st2 = st2.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let n = (st2 >> 52) % 8 + 1;
        let alpha = "0123456789xX-+.e ";
        let mut s = String::new();
        for k in 0..n {
            s.push(alpha.chars().next().unwrap());
            let idx = ((st2 >> (k * 4)) % alpha.len() as u64) as usize;
            s.push(alpha.chars().nth(idx).unwrap());
        }
        let r = run(&[s.as_str()]);
        if s.chars().all(|c| c.is_ascii_digit()) && !s.is_empty() {
            assert_eq!(r.code, 0, "{s:?}");
        } else if !s.chars().any(|c| c.is_ascii_digit()) {
            assert!(r.code == 1 || r.code == 2, "{s:?} -> {}", r.code);
        } else {
            assert!(matches!(r.code, 0 | 1 | 2), "{s:?} -> {}", r.code);
        }
    }
}
