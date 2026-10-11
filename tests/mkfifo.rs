//! mkfifo(1) e2e tests（guest 內執行）。
//!
//! 基準：本機參考 uutils coreutils 0.10.0 pair-diff（14 cases 全一致，docs/probes/cluster5.md）。
use coreutils::testutil as t;
use std::os::unix::fs::PermissionsExt;

fn fresh(tag: &str) -> std::path::PathBuf {
    let d = std::path::PathBuf::from(format!("/tmp/fifo_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::env::set_current_dir(&d).unwrap();
    d
}

fn m(args: &[&str]) -> t::Out {
    t::run("mkfifo", args, "")
}

fn mode_of(p: &str) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o7777
}

/// 以一次無 -m 的 mkfifo 探得 umask（observed = 0o777 & !umask）
fn observed_default_mode(d: &std::path::Path) -> u32 {
    let p = d.join("umask_probe").to_string_lossy().into_owned();
    assert_eq!(m(&[&p]).code, 0);
    let mo = mode_of(&p) & 0o777;
    std::fs::remove_file(&p).unwrap();
    mo
}

fn is_fifo(p: &str) -> bool {
    let m = std::fs::metadata(p).unwrap().file_type();
    m.is_fifo()
}

#[test]
fn simple_and_default_mode() {
    let d = fresh("simple");
    let p = d.join("f1").to_string_lossy().into_owned();
    assert_eq!(m(&[&p]).code, 0);
    assert!(is_fifo(&p));
    assert_eq!(mode_of(&p) & 0o777, observed_default_mode(&d));
    // 多 operand
    let p2 = d.join("f2").to_string_lossy().into_owned();
    let p3 = d.join("f3").to_string_lossy().into_owned();
    assert_eq!(m(&[&p2, &p3]).code, 0);
    assert!(is_fifo(&p2) && is_fifo(&p3));
    // -Z 被忽略（無 SELinux）
    let p4 = d.join("f4").to_string_lossy().into_owned();
    assert_eq!(m(&["-Z", &p4]).code, 0);
    assert!(is_fifo(&p4));
}

#[test]
fn explicit_mode() {
    let d = fresh("mode");
    let p = d.join("m1").to_string_lossy().into_owned();
    assert_eq!(m(&["-m", "644", &p]).code, 0);
    assert_eq!(mode_of(&p) & 0o777, 0o644);
    let p2 = d.join("m2").to_string_lossy().into_owned();
    assert_eq!(m(&["--mode", "600", &p2]).code, 0);
    assert_eq!(mode_of(&p2) & 0o777, 0o600);
    // symbolic：who 省略 → 0o777 & !umask 為基底（與 chmod/install 同語意）
    let observed = observed_default_mode(&d);
    let p3 = d.join("m3").to_string_lossy().into_owned();
    assert_eq!(m(&["-m", "u=rw,g=r,o=", &p3]).code, 0);
    assert_eq!(mode_of(&p3) & 0o777, 0o640);
    let p4 = d.join("m4").to_string_lossy().into_owned();
    assert_eq!(m(&["-m", "a+x", &p4]).code, 0);
    assert_eq!(mode_of(&p4) & 0o777, observed | 0o111);
    // 000 可建立
    let p5 = d.join("m5").to_string_lossy().into_owned();
    assert_eq!(m(&["-m", "000", &p5]).code, 0);
    assert_eq!(mode_of(&p5) & 0o777, 0o000);
}

#[test]
fn errors() {
    let d = fresh("err");
    let p = d.join("e1").to_string_lossy().into_owned();
    assert_eq!(m(&[&p]).code, 0);
    let r = m(&[&p]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), format!("mkfifo: cannot create fifo '{p}': File exists"));
    // 不存在上層
    let bad = d.join("nodir/x").to_string_lossy().into_owned();
    let r = m(&[&bad]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains(&format!("mkfifo: cannot create fifo '{bad}'")), "{}", r.err());
    assert!(r.err().contains("No such file or directory"), "{}", r.err());
    // invalid mode（uutils symbolic-mode 措辭）＋不建立檔案
    let bad2 = d.join("badmode").to_string_lossy().into_owned();
    let r = m(&["-m", "bogus", &bad2]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        "mkfifo: invalid mode: invalid operator (expected +, -, or =, but found b)"
    );
    assert!(!std::path::Path::new(&bad2).exists());
    // 無 operand → clap required-args 措辭
    let r = m(&[]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("error: the following required arguments were not provided:"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("  <fifo>..."), "{}", r.err());
    let r = m(&["-m", "777"]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("<fifo>..."), "{}", r.err());
    // -m 缺值
    let r = m(&["-m"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().lines().next().unwrap(),
        "error: a value is required for '-m <MODE>' but none was supplied"
    );
    let r = t::run("mkfifo", &["--version"], "");
    assert_eq!((r.code, r.out()), (0, "mkfifo (uutils coreutils) 0.10.0\n"));
    let r = t::run("mkfifo", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("create FIFOs (named pipes)"), "{}", r.out());
}

#[test]
fn e2e_binary_dispatch() {
    let d = fresh("e2e");
    let p = d.join("b1").to_string_lossy().into_owned();
    let r = t::run_bin("mkfifo", &[&p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(is_fifo(&p));
}

#[test]
fn fuzz_fifo_type_and_mode() {
    let d = fresh("fz");
    let mut rng = t::Rng::new(20261102);
    for k in 0..30 {
        let mode = 0o777u32 & !(rng.below(0o1000) as u32);
        let p = d.join(format!("fz_{k}")).to_string_lossy().into_owned();
        let mspec = format!("{:03o}", mode);
        let r = m(&["-m", &mspec, &p]);
        assert_eq!(r.code, 0, "case {k}: {}", r.err());
        assert!(is_fifo(&p), "case {k} must be a fifo");
        // 顯式 -m 恒確立（umask 不影響）
        assert_eq!(mode_of(&p) & 0o7777, mode, "case {k} mode {}", mspec);
        // 已存在 → File exists
        let r = m(&[&p]);
        assert_eq!(r.code, 1, "case {k} duplicate");
        assert!(r.err().contains("File exists"), "case {k}: {}", r.err());
    }
}
