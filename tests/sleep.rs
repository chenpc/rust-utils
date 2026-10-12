//! sleep(1) guest e2e 測試。
//!
//! 斷言依據：docs/man/sleep.1.gz（`SUFFIX may be 's' (default), 'm', 'h' or 'd'`、
//! `NUMBER may be an arbitrary floating point number`、`Given two or more arguments,
//! pause for the sum of their values`）＋ man 未載明時由本機參考實測決定的部分
//! （錯誤措辭、exit code），見 docs/assumptions.md。
//!
//! 時間Sensitive：測試只用 0 或極小的間隔，避免拖慢 guest；真正「有睡」的驗證用
//! Instant 量測下限。

use coreutils::testutil as t;
use std::time::{Duration, Instant};

fn run(args: &[&str]) -> t::Out {
    t::run("sleep", args, "")
}

#[test]
fn zero_intervals_return_immediately() {
    for arg in [
        "0", "0s", "0m", "0h", "0d", "0.0", ".0", "+0", "0.000", "0e0",
    ] {
        let r = run(&[arg]);
        assert_eq!(r.code, 0, "{}: {}", arg, r.err());
        assert_eq!(r.out(), "");
    }
}

#[test]
fn suffix_multipliers_and_sum_are_accepted() {
    // man: 兩個以上 operand = 總和；這裡只用 0 與極小值
    let cases: &[&[&str]] = &[
        &["0", "0"],
        &["0s", "0m", "0h", "0d"],
        &["0.01"],
        &["0.01s", "0.01"],
        &["0.001m"],
        &["+0", "0"],
        &["0", "--", "0"],
        &["--", "0", "0"],
    ];
    for c in cases {
        let r = run(c);
        assert_eq!(r.code, 0, "{:?}: {}", c, r.err());
        assert_eq!(r.out(), "");
        assert_eq!(r.err(), "");
    }
}

#[test]
fn actually_sleeps_for_the_sum() {
    let start = Instant::now();
    let r = run(&["0.2", "0.3s"]);
    let elapsed = start.elapsed();
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(
        elapsed >= Duration::from_millis(450),
        "0.2 + 0.3s 只睡了 {:?}",
        elapsed
    );
    // 上限只是防止測試卡死；0.5s 不該睡到 5s
    assert!(elapsed < Duration::from_secs(5), "{:?}", elapsed);
}

#[test]
fn invalid_intervals() {
    let bad = [
        "abc", "1x", "1M", "1S", "1D", "1m30", "1 s", "", "nan", "-",
        "1e", "..", "1,5", "s", "m", "5h3",
    ];
    for arg in bad {
        let r = run(&[arg]);
        assert_eq!(r.code, 1, "{}", arg);
        assert!(
            r.err()
                .starts_with(&format!("sleep: invalid time interval '{}'", arg)),
            "{} → {}",
            arg,
            r.err()
        );
    assert!(r.err().contains("Try '"), "{} → {}", arg, r.err());
        assert_eq!(r.out(), "");
    }
    // `--` 之後的 token 仍是 operand，照樣走 invalid time interval 措辭
    let r = t::run("sleep", &["--", "-1"], "");
    assert_eq!(r.code, 1);
    assert!(
        r.err()
            .starts_with("sleep: invalid time interval '-1'"),
        "{}",
        r.err()
    );
    // 反而出現「看起來像選項」的單一 token（`-- -1` 帶空白）時是 clap 錯誤
    let r = t::run("sleep", &["-- -1"], "");
    assert_eq!(r.code, 1);
    assert!(
        r.err()
            .starts_with("error: unexpected argument '-- -1' found"),
        "{}",
        r.err()
    );
    // 十六進位浮點：REF（C strtod）接受 `0x10` = 16 會真的睡；我方一律拒絕
    // （記錄在案的偏差，見 docs/assumptions.md）。
    for arg in ["0x10", "0x10s"] {
        let r = run(&[arg]);
        assert_eq!(r.code, 1, "{}", arg);
        assert!(
            r.err()
                .starts_with(&format!("sleep: invalid time interval '{}'", arg)),
            "{} → {}",
            arg,
            r.err()
        );
    }
}

#[test]
fn missing_operand() {
    let r = run(&[]);
    assert_eq!(r.code, 1);
    assert_eq!(r.out(), "");
    assert!(r.err().starts_with("sleep: missing operand\n"), "{}", r.err());
    assert!(r.err().contains("Try '"));
    // `--` 之後沒有任何 operand 也算缺少 operand
    let r2 = run(&["--"]);
    assert_eq!(r2.code, 1);
    assert!(r2.err().starts_with("sleep: missing operand\n"), "{}", r2.err());
}

#[test]
fn unknown_option_is_clap_style_rc1() {
    for args in [&["--bogus"][..], &["-x"][..], &["-1"][..], &["0", "--nope"][..]] {
        let r = t::run("sleep", args, "");
        assert_eq!(r.code, 1, "{:?}", args);
        let err = r.err();
        let lines: Vec<&str> = err.lines().collect();
        assert!(lines[0].starts_with("error: unexpected argument '"), "{}", err);
        assert_eq!(lines[1], "");
        assert!(err.contains("  tip: to pass '"), "{}", err);
        assert!(err.contains("Usage: sleep NUMBER[SUFFIX]..."), "{}", err);
        assert!(err.contains("       sleep OPTION"), "{}", err);
        assert!(err.contains("For more information, try '--help'."), "{}", err);
    }
    // 以 '-' 開頭的數字不是 operand（實測參考 binary 也報 clap 錯誤）。
    // 已知外觀偏差：clap 對 `-0` 印的參數名是 '0'（不含 dash），我方印 '-0'；
    // 記於 docs/assumptions.md。
    for args in [&["-0"][..], &["-0x1"][..]] {
        let r = t::run("sleep", args, "");
        assert_eq!(r.code, 1, "{:?}", args);
        assert!(r.err().starts_with("error: unexpected argument '"), "{:?}", args);
        assert!(r.err().contains("tip: to pass '-0'"), "{:?}", args);
    }
}

#[test]
fn options_win_even_after_positional() {
    let r = t::run("sleep", &["0", "--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().starts_with("Pause for NUMBER seconds.\n"), "{}", r.out());
    assert!(r.out().contains("  -h, --help     Print help"));

    let r = t::run("sleep", &["--help", "0"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().starts_with("Pause for NUMBER seconds."));

    let r = t::run("sleep", &["0", "-V"], "");
    assert_eq!(r.out().trim(), "sleep (uutils coreutils) 0.10.0");
    let r = t::run("sleep", &["--vers"], "");
    assert_eq!(r.out().trim(), "sleep (uutils coreutils) 0.10.0");
    let r = t::run("sleep", &["--hel"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().starts_with("Pause for NUMBER seconds."));
    // -h 在短旗標簇中先贏
    let r = t::run("sleep", &["-hV"], "");
    assert!(r.out().starts_with("Pause for NUMBER seconds."));
    let r = t::run("sleep", &["-Vh"], "");
    assert_eq!(r.out().trim(), "sleep (uutils coreutils) 0.10.0");
    // help/version 之後的重複不會觸發 clap 的重複錯誤
    let r = t::run("sleep", &["--help", "--help"], "");
    assert!(r.out().starts_with("Pause for NUMBER seconds."));
}

#[test]
fn usage_and_version_text() {
    let r = t::run("sleep", &["--version"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "sleep (uutils coreutils) 0.10.0\n");
    let r = t::run("sleep", &["--help"], "");
    let out = r.out();
    assert!(out.contains("Usage: sleep NUMBER[SUFFIX]...\n       sleep OPTION\n"), "{}", out);
    assert!(
        out.contains("  -h, --help     Print help\n  -V, --version  Print version\n"),
        "{}",
        out
    );
    assert!(out.contains("NUMBER may be an arbitrary floating"), "{}", out);
    assert!(
        out.contains("Given two or more arguments, pause for the amount of time"),
        "{}",
        out
    );
    assert!(out.contains("  [NUMBER]...  pause for NUMBER seconds"), "{}", out);
    assert!(out.ends_with("specified by the sum of their values.\n"), "{}", out);
}

#[test]
fn run_bin_e2e() {
    let r = t::run_bin("sleep", &["0"], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let r = t::run_bin("sleep", &[], "");
    assert_eq!(r.code, 1);
    assert!(r.err().starts_with("sleep: missing operand"));
}

#[test]
fn fuzz_intervals() {
    let mut rng = t::Rng::new(20261125);
    let suffixes = ["", "s", "m", "h", "d"];
    // 合法且「睡極短」的構造：整數部分一律 0
    let tiny = ["0", "0.0", ".0", "0.000", "+0", "0e0"];
    for round in 0..30 {
        let num = tiny[rng.below(tiny.len() as u64) as usize];
        let sfx = suffixes[rng.below(suffixes.len() as u64) as usize];
        let arg = format!("{}{}", num, sfx);
        let r = run(&[&arg]);
        assert_eq!(r.code, 0, "round {}: {} → {}", round, arg, r.err());
        assert_eq!(r.err(), "");
        assert_eq!(r.out(), "");
    }
    // 0.0–0.9 只配 s/無尾綴（最多睡 0.9s）
    for round in 0..8 {
        let arg = format!("0.{}", rng.below(10));
        let sfx = ["", "s"][rng.below(2) as usize];
        let arg = format!("{}{}", arg, sfx);
        let r = run(&[&arg]);
        assert_eq!(r.code, 0, "round {}: {} → {}", round, arg, r.err());
    }
    // 亂塞字母尾綴一律 invalid（尾綴字母刻意排除 s/m/h/d，避免真的睡幾秒以上）
    let bad_letters = b"abc efijklnopqrtuvwxyz";
    for _ in 0..12 {
        let c = bad_letters[rng.below(bad_letters.len() as u64) as usize] as char;
        if c == ' ' {
            continue;
        }
        let arg = format!("1{}", c);
        let r = run(&[&arg]);
        assert_eq!(r.code, 1, "{}", arg);
        assert!(
            r.err()
                .starts_with(&format!("sleep: invalid time interval '{}'", arg)),
            "{} → {}",
            arg,
            r.err()
        );
    }
    // 多個 operand 全部為 0（不會睡）
    for round in 0..6 {
        let n = 2 + rng.below(3);
        let mut args: Vec<String> = Vec::new();
        for _ in 0..n {
            let num = tiny[rng.below(tiny.len() as u64) as usize];
            let sfx = suffixes[rng.below(suffixes.len() as u64) as usize];
            args.push(format!("{}{}", num, sfx));
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let r = run(&refs);
        assert_eq!(r.code, 0, "round {}: {:?} → {}", round, args, r.err());
    }
}
