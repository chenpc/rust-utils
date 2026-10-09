//! cat(1) e2e — 行為依據：docs/man/rust-cat.1.gz（本機 mirror）
//!
//! 這些測試只在 qemu guest 內跑（scripts/guest_test.sh cat）。

use coreutils::testutil::*;

#[test]
fn no_args_copies_stdin() {
    let r = run("cat", &[], "hello\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "hello\n");
    assert_eq!(r.err(), "");
}

#[test]
fn dash_reads_stdin() {
    let r = run("cat", &["-"], "from stdin\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "from stdin\n");
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = run("cat", &["/tmp/definitely-not-here"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("cat:"), "stderr should start with cat: ({0})", r.err());
    assert!(r.err().contains("definitely-not-here"));
}

#[test]
fn missing_file_does_not_stop_remaining_args() {
    let w = workdir();
    let f = w.join("cat_ok.txt");
    std::fs::write(&f, "second\n").unwrap();
    let r = run("cat", &["/tmp/nope", &f.to_string_lossy()], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.out(), "second\n");
    std::fs::remove_file(&f).ok();
}

#[test]
fn file_is_copied_verbatim() {
    let w = workdir();
    let f = w.join("cat_verbatim.txt");
    let data = "line1\nline2\n";
    std::fs::write(&f, data).unwrap();
    let r = run("cat", &[&f.to_string_lossy()], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), data);
    std::fs::remove_file(&f).ok();
}

#[test]
fn e2e_binary_dispatch() {
    // argv[0] 是 binary 本身 → applet 名稱來自第一個參數
    let r = run_bin("cat", &[], "e2e\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "e2e\n");
}

#[test]
fn write_error_on_stdin_returns_1() {
    let mut w = FailWriter::new(0);
    let mut err = Vec::new();
    let code = run_with("cat", &[], b"abc", &mut w, &mut err);
    assert_eq!(code, 1);
    assert!(String::from_utf8_lossy(&err).contains("standard input"), "{0}", String::from_utf8_lossy(&err));
}

#[test]
fn write_error_on_file_copy_returns_1() {
    let w = workdir();
    let f = w.join("cat_write_err.txt");
    std::fs::write(&f, "data").unwrap();
    let mut out = FailWriter::new(0);
    let mut err = Vec::new();
    let code = run_with("cat", &[&f.to_string_lossy()], b"", &mut out, &mut err);
    assert_eq!(code, 1);
    assert!(String::from_utf8_lossy(&err).contains("cat:"), "{0}", String::from_utf8_lossy(&err));
    std::fs::remove_file(&f).ok();
}

#[test]
fn write_error_on_dash_stdin_returns_1() {
    let mut w = FailWriter::new(0);
    let mut err = Vec::new();
    let code = run_with("cat", &["-"], b"abc", &mut w, &mut err);
    assert_eq!(code, 1);
    assert!(String::from_utf8_lossy(&err).contains("standard input"), "{0}", String::from_utf8_lossy(&err));
}

#[test]
fn fuzz_identity_invariant() {
    // man page 的不變量：cat 沒有選項時是「逐 byte 複製」，所以輸出必須與輸入完全相同。
    let mut rng = Rng::new(0xC0DE);
    for i in 0..200 {
        let n = 1 + (rng.below(400) as usize);
        let raw = rng.bytes(n);
        let r = run_bytes("cat", &[], &raw);
        assert_eq!(r.code, 0, "iteration {i}");
        assert_eq!(r.stdout, raw, "iteration {i}: cat must be byte-identity");
    }
}

#[test]
fn fuzz_file_matches_input() {
    let mut rng = Rng::new(0xFEED);
    let w = workdir();
    for i in 0..100 {
        let n = 1 + (rng.below(200) as usize);
        let raw = rng.bytes(n);
        let f = w.join(format!("fuzz_{i}.bin"));
        std::fs::write(&f, &raw).unwrap();
        let r = run("cat", &[&f.to_string_lossy()], "");
        assert_eq!(r.code, 0, "iteration {i}");
        assert_eq!(r.stdout, raw, "iteration {i}");
        std::fs::remove_file(&f).ok();
    }
}
