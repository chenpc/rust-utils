use coreutils::testutil as t;

#[test]
fn default_keeps_first_of_each_group() {
    let r = t::run("uniq", &[], "a\na\nb\nb\nb\n");
    assert_eq!(r.out(), "a\nb\n");
}

#[test]
fn count_prefix() {
    let r = t::run("uniq", &["-c"], "a\na\nb\n");
    assert_eq!(r.out(), "      2 a\n      1 b\n");
}

#[test]
fn repeated_only() {
    let r = t::run("uniq", &["-d"], "a\na\nb\n");
    assert_eq!(r.out(), "a\n");
}

#[test]
fn unique_only() {
    let r = t::run("uniq", &["-u"], "a\na\nb\n");
    assert_eq!(r.out(), "b\n");
}

#[test]
fn all_repeated() {
    let r = t::run("uniq", &["-D"], "a\na\nb\n");
    assert_eq!(r.out(), "a\na\n");
}

#[test]
fn all_repeated_separate() {
    let r = t::run("uniq", &["-D", "--all-repeated=separate"], "a\na\nb\nb\n");
    assert_eq!(r.out(), "a\na\n\nb\nb\n");
}

#[test]
fn group_separate() {
    let r = t::run("uniq", &["--group"], "a\na\nb\n");
    assert_eq!(r.out(), "a\na\n\nb\n");
}

#[test]
fn ignore_case() {
    let r = t::run("uniq", &["-i"], "A\na\nb\n");
    assert_eq!(r.out(), "A\nb\n");
}

#[test]
fn skip_chars() {
    let r = t::run("uniq", &["-s", "2"], "xxa\nyya\nb\n");
    assert_eq!(r.out(), "xxa\nb\n");
}

#[test]
fn skip_fields() {
    let r = t::run("uniq", &["-f", "1"], "1 a\n2 a\n3 b\n");
    assert_eq!(r.out(), "1 a\n3 b\n");
}

#[test]
fn check_chars() {
    let r = t::run("uniq", &["-w", "1"], "ab\nac\nb\n");
    assert_eq!(r.out(), "ab\nb\n");
}

#[test]
fn zero_terminated() {
    let r = t::run_bytes("uniq", &["-z"], b"a\0a\0b\0");
    assert_eq!(r.out_bytes(), b"a\0b\0");
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = t::run("uniq", &["/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("uniq: /nonexistent/x"));
}

#[test]
fn help_and_version() {
    let r = t::run("uniq", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("uniq"));
    let v = t::run("uniq", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("uniq"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("uniq", &["-c"], "a\na\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "      2 a\n");
}

#[test]
fn write_error_returns_1() {
    let mut w = t::FailWriter::new(2);
    let mut e = Vec::new();
    let code = t::run_with("uniq", &[], b"a\nb\nc\n", &mut w, &mut e);
    assert_eq!(code, 1);
}

#[test]
fn fuzz_no_adjacent_duplicates_in_default_mode() {
    let mut rng = t::Rng::new(0x5151);
    for _ in 0..120 {
        let n = 1 + rng.below(12);
        let mut lines = Vec::new();
        for _ in 0..n {
            let s: String = (0..rng.below(4))
                .map(|_| char::from_u32(0x21 + rng.below(90) as u32).unwrap())
                .collect();
            lines.push(s);
        }
        let input = lines.join("\n") + "\n";
        let r = t::run("uniq", &[], &input);
        assert_eq!(r.code, 0);
        let o = r.out();
        let got: Vec<&str> = o.lines().collect();
        for i in 1..got.len() {
            assert_ne!(got[i - 1], got[i], "input={input}");
        }
        // every output line must appear in the input, in order
        let mut src = 0;
        for g in &got {
            while src < lines.len() && lines[src] != *g {
                src += 1;
            }
            assert!(src < lines.len(), "output line {g} not from input order");
            src += 1;
        }
    }
}

#[test]
fn fuzz_count_sums_to_input_lines() {
    let mut rng = t::Rng::new(0xB055);
    for _ in 0..100 {
        let n = 1 + rng.below(15);
        let mut lines = Vec::new();
        for _ in 0..n {
            let s: String = (0..rng.below(3))
                .map(|_| char::from_u32(0x21 + rng.below(60) as u32).unwrap())
                .collect();
            lines.push(s);
        }
        let input = lines.join("\n") + "\n";
        let r = t::run("uniq", &["-c"], &input);
        assert_eq!(r.code, 0);
        let o = r.out();
        let mut total = 0usize;
        for l in o.lines() {
            let c: usize = l.split_whitespace().next().unwrap().parse().unwrap();
            total += c;
        }
        assert_eq!(total, lines.len(), "input={input} out={o}");
    }
}
