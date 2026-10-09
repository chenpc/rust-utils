use std::path::PathBuf;

use coreutils::testutil as t;

fn tmp(name: &str) -> PathBuf {
    t::workdir().join(name)
}

#[test]
fn copies_stdin_to_stdout_and_file() {
    let f = tmp("tee_a.txt");
    let r = t::run("tee", &[f.to_str().unwrap()], "hello\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "hello\n");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "hello\n");
}

#[test]
fn writes_to_multiple_files() {
    let a = tmp("tee_b1.txt");
    let b = tmp("tee_b2.txt");
    let r = t::run("tee", &[a.to_str().unwrap(), b.to_str().unwrap()], "x\ny\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "x\ny\n");
    assert_eq!(std::fs::read_to_string(&a).unwrap(), "x\ny\n");
    assert_eq!(std::fs::read_to_string(&b).unwrap(), "x\ny\n");
}

#[test]
fn append_does_not_truncate() {
    let f = tmp("tee_c.txt");
    std::fs::write(&f, "old\n").unwrap();
    let r = t::run("tee", &["-a", f.to_str().unwrap()], "new\n");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "old\nnew\n");
}

#[test]
fn without_append_truncates() {
    let f = tmp("tee_d.txt");
    std::fs::write(&f, "old\n").unwrap();
    let r = t::run("tee", &[f.to_str().unwrap()], "new\n");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "new\n");
}

#[test]
fn dash_is_a_file_named_dash() {
    let f = t::workdir().join("-");
    let r = t::run("tee", &[f.to_str().unwrap()], "z\n");
    assert_eq!(r.code, 0);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "z\n");
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = t::run("tee", &["/nonexistent/x"], "a\n");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("tee: /nonexistent/x"));
}

#[test]
fn write_error_on_stdout_returns_1() {
    let mut w = t::FailWriter::new(1);
    let mut e = Vec::new();
    let code = t::run_with("tee", &[], b"abc\n", &mut w, &mut e);
    assert_eq!(code, 1);
}

#[test]
fn output_error_exit_stops_immediately() {
    let mut w = t::FailWriter::new(1);
    let mut e = Vec::new();
    let code = t::run_with("tee", &["--output-error", "exit"], b"abc\n", &mut w, &mut e);
    assert_eq!(code, 1);
}

#[test]
fn output_error_warn_continues() {
    let f = tmp("tee_warn.txt");
    let mut w = t::FailWriter::new(1);
    let mut e = Vec::new();
    let code = t::run_with("tee", &["--output-error", "warn", f.to_str().unwrap()], b"abc\n", &mut w, &mut e);
    assert_eq!(code, 1);
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "abc\n");
}

#[test]
fn unknown_option_is_exit_2() {
    let r = t::run("tee", &["-x"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("unknown option"));
}

#[test]
fn help_and_version() {
    let r = t::run("tee", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("tee"));
    let v = t::run("tee", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("tee"));
}

#[test]
fn e2e_binary_dispatch() {
    let f = tmp("tee_e2e.txt");
    let r = t::run_bin("tee", &[f.to_str().unwrap()], "q\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "q\n");
    assert_eq!(std::fs::read_to_string(&f).unwrap(), "q\n");
}

#[test]
fn fuzz_copy_is_verbatim() {
    let mut rng = t::Rng::new(0xBEEF);
    for i in 0..60 {
        let n = rng.below(40);
        let data = rng.bytes(n as usize);
        let f = tmp(&format!("tee_fuzz_{i}.txt"));
        let r = t::run_bytes("tee", &[f.to_str().unwrap()], &data);
        assert_eq!(r.code, 0);
        assert_eq!(r.out_bytes(), data, "stdout must equal input");
        assert_eq!(std::fs::read(&f).unwrap(), data, "file must equal input");
    }
}

#[test]
fn fuzz_append_accumulates() {
    let mut rng = t::Rng::new(0xC0FFEE);
    let f = tmp("tee_fuzz_append.txt");
    std::fs::write(&f, b"").unwrap();
    let mut expected = Vec::new();
    for _ in 0..20 {
        let n = 1 + rng.below(20);
        let data = rng.bytes(n as usize);
        let r = t::run_bytes("tee", &["-a", f.to_str().unwrap()], &data);
        assert_eq!(r.code, 0);
        expected.extend_from_slice(&data);
        assert_eq!(std::fs::read(&f).unwrap(), expected);
    }
}
