use coreutils::testutil as t;

#[test]
fn default_is_last_10_lines() {
    let input = (1..=12).map(|i| format!("{i}\n")).collect::<String>();
    let r = t::run("tail", &[], &input);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), (3..=12).map(|i| format!("{i}\n")).collect::<String>());
}

#[test]
fn lines_option() {
    let r = t::run("tail", &["-n", "2"], "a\nb\nc\n");
    assert_eq!(r.out(), "b\nc\n");
}

#[test]
fn bytes_option() {
    let r = t::run("tail", &["-c", "4"], "abcdef\n");
    assert_eq!(r.out(), "def\n");
}

#[test]
fn plus_k_starts_at_kth_line() {
    let r = t::run("tail", &["-n", "+3"], "a\nb\nc\nd\n");
    assert_eq!(r.out(), "c\nd\n");
}

#[test]
fn negative_k_prints_all_but_first_k() {
    let r = t::run("tail", &["-n", "-2"], "a\nb\nc\nd\n");
    assert_eq!(r.out(), "c\nd\n");
}

#[test]
fn negative_bytes_prints_all_but_first_k() {
    let r = t::run("tail", &["-c", "-3"], "abcdef\n");
    assert_eq!(r.out(), "def\n");
}

#[test]
fn header_with_multiple_files() {
    let w = t::workdir();
    std::fs::write(w.join("t1.txt"), "1\n").unwrap();
    std::fs::write(w.join("t2.txt"), "2\n").unwrap();
    let r = t::run("tail", &[w.join("t1.txt").to_str().unwrap(), w.join("t2.txt").to_str().unwrap()], "");
    assert_eq!(r.out(), format!("==> {} <==\n1\n==> {} <==\n2\n", w.join("t1.txt").display(), w.join("t2.txt").display()));
}

#[test]
fn quiet_suppresses_header() {
    let w = t::workdir();
    std::fs::write(w.join("t3.txt"), "1\n").unwrap();
    std::fs::write(w.join("t4.txt"), "2\n").unwrap();
    let r = t::run("tail", &["-q", w.join("t3.txt").to_str().unwrap(), w.join("t4.txt").to_str().unwrap()], "");
    assert_eq!(r.out(), "1\n2\n");
}

#[test]
fn zero_terminated() {
    let r = t::run("tail", &["-n", "2", "-z"], "a\0b\0c\0");
    assert_eq!(r.out(), "b\0c\0");
}

#[test]
fn follow_is_single_pass() {
    let r = t::run("tail", &["-f", "-n", "1"], "a\nb\n");
    assert_eq!(r.out(), "b\n");
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = t::run("tail", &["/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("tail: /nonexistent/x"));
}

#[test]
fn help_and_version() {
    let r = t::run("tail", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("tail"));
    let v = t::run("tail", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("tail"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("tail", &["-n", "1"], "a\nb\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "b\n");
}

#[test]
fn write_error_returns_1() {
    let mut w = t::FailWriter::new(1);
    let mut e = Vec::new();
    let code = t::run_with("tail", &["-c", "5"], b"abc\n", &mut w, &mut e);
    assert_eq!(code, 1);
}

#[test]
fn fuzz_tail_is_suffix() {
    let mut rng = t::Rng::new(0xB2);
    for _ in 0..100 {
        let n = 1 + rng.below(20);
        let lines: Vec<String> = (0..n)
            .map(|_| {
                let len = rng.below(6);
                let s: String = (0..len)
                    .map(|_| char::from_u32(0x21 + rng.below(90) as u32).unwrap())
                    .collect();
                s
            })
            .collect();
        let input = lines.join("\n") + "\n";
        let k = 1 + rng.below(n as u64);
        let r = t::run("tail", &["-n", &k.to_string()], &input);
        assert_eq!(r.code, 0);
        let expected = lines[(n - k) as usize..].join("\n") + "\n";
        assert_eq!(r.out(), expected, "input={input} k={k}");
    }
}
