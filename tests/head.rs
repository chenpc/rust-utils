use coreutils::testutil as t;

#[test]
fn default_is_first_10_lines() {
    let input = (1..=12).map(|i| format!("{i}\n")).collect::<String>();
    let r = t::run("head", &[], &input);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), (1..=10).map(|i| format!("{i}\n")).collect::<String>());
}

#[test]
fn lines_option() {
    let r = t::run("head", &["-n", "2"], "a\nb\nc\n");
    assert_eq!(r.out(), "a\nb\n");
}

#[test]
fn bytes_option() {
    let r = t::run("head", &["-c", "3"], "abcdef\n");
    assert_eq!(r.out(), "abc");
}

#[test]
fn negative_lines_prints_all_but_last() {
    let r = t::run("head", &["-n", "-2"], "a\nb\nc\nd\n");
    assert_eq!(r.out(), "a\nb\n");
}

#[test]
fn negative_bytes_prints_all_but_last() {
    let r = t::run("head", &["-c", "-3"], "abcdef\n");
    assert_eq!(r.out(), "abcd");
}

#[test]
fn header_with_multiple_files() {
    let w = t::workdir();
    std::fs::write(w.join("h1.txt"), "1\n").unwrap();
    std::fs::write(w.join("h2.txt"), "2\n").unwrap();
    let r = t::run("head", &[w.join("h1.txt").to_str().unwrap(), w.join("h2.txt").to_str().unwrap()], "");
    assert_eq!(r.out(), format!("==> {} <==\n1\n==> {} <==\n2\n", w.join("h1.txt").display(), w.join("h2.txt").display()));
}

#[test]
fn quiet_suppresses_header() {
    let w = t::workdir();
    std::fs::write(w.join("h3.txt"), "1\n").unwrap();
    std::fs::write(w.join("h4.txt"), "2\n").unwrap();
    let r = t::run("head", &["-q", w.join("h3.txt").to_str().unwrap(), w.join("h4.txt").to_str().unwrap()], "");
    assert_eq!(r.out(), "1\n2\n");
}

#[test]
fn verbose_always_prints_header() {
    let w = t::workdir();
    std::fs::write(w.join("h5.txt"), "1\n").unwrap();
    let r = t::run("head", &["-v", w.join("h5.txt").to_str().unwrap()], "");
    assert_eq!(r.out(), format!("==> {} <==\n1\n", w.join("h5.txt").display()));
}

#[test]
fn zero_terminated() {
    let r = t::run("head", &["-n", "2", "-z"], "a\0b\0c\0");
    assert_eq!(r.out(), "a\0b\0");
}

#[test]
fn dash_reads_stdin() {
    let r = t::run("head", &["-n", "1", "-"], "x\ny\n");
    assert_eq!(r.out(), "x\n");
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = t::run("head", &["/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("head: /nonexistent/x"));
}

#[test]
fn invalid_number_is_exit_2() {
    let r = t::run("head", &["-n", "x"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("invalid number"));
}

#[test]
fn help_and_version() {
    let r = t::run("head", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("head"));
    let v = t::run("head", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("head"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("head", &["-n", "1"], "a\nb\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "a\n");
}

#[test]
fn write_error_returns_1() {
    let mut w = t::FailWriter::new(1);
    let mut e = Vec::new();
    let code = t::run_with("head", &["-c", "5"], b"abc\n", &mut w, &mut e);
    assert_eq!(code, 1);
}

#[test]
fn fuzz_head_is_prefix() {
    let mut rng = t::Rng::new(0xA1);
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
        let r = t::run("head", &["-n", &k.to_string()], &input);
        assert_eq!(r.code, 0);
        let expected = lines[..k as usize].join("\n") + "\n";
        assert_eq!(r.out(), expected, "input={input} k={k}");
    }
}
