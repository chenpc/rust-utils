use coreutils::testutil as t;

fn sort(args: &[&str], input: &str) -> t::Out {
    t::run("sort", args, input)
}

#[test]
fn lexicographic_sort() {
    let r = sort(&[], "b\na\nc\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "a\nb\nc\n");
}

#[test]
fn empty_lines_are_kept() {
    let r = sort(&[], "b\n\na\n");
    assert_eq!(r.out(), "\na\nb\n");
}

#[test]
fn numeric_sort() {
    assert_eq!(sort(&["-n"], "10\n9\n2\n").out(), "2\n9\n10\n");
}

#[test]
fn general_numeric_sort() {
    let r = sort(&["-g"], "1e3\n2\n100\n");
    assert_eq!(r.out(), "2\n100\n1e3\n");
}

#[test]
fn human_numeric_sort() {
    let r = sort(&["-h"], "1M\n100k\n2K\n");
    assert_eq!(r.out(), "2K\n100k\n1M\n");
}

#[test]
fn month_sort() {
    let r = sort(&["-M"], "Jan\nMar\nFeb\n");
    assert_eq!(r.out(), "Jan\nFeb\nMar\n");
}

#[test]
fn version_sort() {
    let r = sort(&["-V"], "1.12.2\n1.1.2\n1.2.10\n");
    assert_eq!(r.out(), "1.1.2\n1.2.10\n1.12.2\n");
}

#[test]
fn random_sort_is_deterministic() {
    let a = sort(&["-R"], "foo\nbar\nbaz\nqux\n");
    let b = sort(&["-R"], "foo\nbar\nbaz\nqux\n");
    assert_eq!(a.out(), b.out());
    assert_ne!(a.out(), "bar\nbaz\nfoo\nqux\n");
}

#[test]
fn key_by_field() {
    let r = sort(&["-k2,2"], "x 3\ny 1\nz 2\n");
    assert_eq!(r.out(), "y 1\nz 2\nx 3\n");
}

#[test]
fn key_with_custom_separator() {
    let r = sort(&["-t", ",", "-k2,2"], "a,b\nc,a\nb,b\n");
    assert_eq!(r.out(), "c,a\na,b\nb,b\n");
}

#[test]
fn key_char_positions() {
    let r = sort(&["-k1.2,1.2"], "ba\nab\n");
    assert_eq!(r.out(), "ab\nba\n");
}

#[test]
fn key_open_end() {
    let r = sort(&["-k2"], "a z\nb y\n");
    assert_eq!(r.out(), "b y\na z\n");
}

#[test]
fn key_options_override_global() {
    // -k2,2n sorts field 2 numerically even though the global mode is lexicographic
    let r = sort(&["-k2,2n"], "a 10\nb 2\n");
    assert_eq!(r.out(), "b 2\na 10\n");
}

#[test]
fn key_reverse_option() {
    let r = sort(&["-k1,1r"], "a\nb\nc\n");
    assert_eq!(r.out(), "c\nb\na\n");
}

#[test]
fn ignore_leading_blanks() {
    let r = sort(&["-k1,1b"], "  b\na\n");
    assert_eq!(r.out(), "a\n  b\n");
}

#[test]
fn dictionary_order() {
    let r = sort(&["-d"], "a!b\naa\n");
    assert_eq!(r.out(), "aa\na!b\n");
}

#[test]
fn ignore_case() {
    let r = sort(&["-f"], "B\na\n");
    assert_eq!(r.out(), "a\nB\n");
}

#[test]
fn ignore_nonprinting() {
    let r = sort(&["-i"], "a\x01b\naa\n");
    assert_eq!(r.out(), "aa\na\x01b\n");
}

#[test]
fn reverse() {
    assert_eq!(sort(&["-r"], "a\nb\nc\n").out(), "c\nb\na\n");
}

#[test]
fn stable_keeps_input_order_for_equal_keys() {
    let r = sort(&["-k1,1", "-s"], "a 2\na 1\n");
    assert_eq!(r.out(), "a 2\na 1\n");
}

#[test]
fn last_resort_comparison_when_keys_equal() {
    let r = sort(&["-k1,1"], "b 1\na 1\n");
    assert_eq!(r.out(), "a 1\nb 1\n");
}

#[test]
fn unique_first_of_equal_run() {
    let r = sort(&["-k1,1", "-u"], "b x\nb y\na z\n");
    assert_eq!(r.out(), "a z\nb x\n");
}

#[test]
fn check_detects_disorder() {
    let r = sort(&["-c"], "b\na\n");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("disorder"));
    let ok = sort(&["-c"], "a\nb\n");
    assert_eq!(ok.code, 0);
}

#[test]
fn check_silent() {
    assert_eq!(sort(&["-C"], "b\na\n").code, 1);
    assert_eq!(sort(&["-C"], "a\nb\n").code, 0);
}

#[test]
fn merge_is_stable_sort() {
    let r = sort(&["-m"], "a\nc\nb\n");
    assert_eq!(r.out(), "a\nb\nc\n");
}

#[test]
fn zero_terminated() {
    let r = t::run_bytes("sort", &["-z"], b"b\0a\0");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, b"a\0b\0");
}

#[test]
fn output_to_file() {
    let dir = t::workdir();
    let path = dir.join("sort_out.txt");
    let p = path.to_str().unwrap();
    let r = sort(&["-o", p], "b\na\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "");
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "a\nb\n");
}

#[test]
fn missing_file_exit_1() {
    let r = sort(&["/definitely/not/here"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("sort:"));
}

#[test]
fn invalid_key_and_unknown_option_exit_2() {
    assert_eq!(sort(&["-k", "x"], "a\n").code, 2);
    assert_eq!(sort(&["-Z"], "a\n").code, 2);
    assert_eq!(sort(&["-o"], "a\n").code, 2);
}

#[test]
fn help_and_version() {
    let r = sort(&["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("sort"));
    let v = sort(&["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.out().contains("sort"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("sort", &["-n"], "3\n1\n2\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "1\n2\n3\n");
}

#[test]
fn fuzz_output_is_sorted_and_preserves_lines() {
    let mut rng = t::Rng::new(0x50DE);
    for _ in 0..100 {
        let n = 2 + rng.below(12);
        let mut lines: Vec<String> = Vec::new();
        for _ in 0..n {
            let len = 1 + rng.below(6);
            let mut w = String::new();
            for _ in 0..len {
                w.push((0x61 + rng.below(20)) as u8 as char);
            }
            lines.push(w);
        }
        let input = lines.join("\n") + "\n";
        let r = sort(&[], &input);
        assert_eq!(r.code, 0);
        let out = r.out();
        let got: Vec<&str> = out.lines().collect();
        let mut want = lines.clone();
        want.sort();
        assert_eq!(got, want);
    }
}

#[test]
fn fuzz_numeric_key_order() {
    let mut rng = t::Rng::new(0xB10E);
    for _ in 0..60 {
        let n = 2 + rng.below(10);
        let mut rows: Vec<(String, i64)> = Vec::new();
        for _ in 0..n {
            let v = rng.below(1000) as i64;
            rows.push((format!("k{}", rng.below(100)), v));
        }
        let input = rows.iter().map(|(k, v)| format!("{} {}", k, v)).collect::<Vec<String>>().join("\n") + "\n";
        let r = sort(&["-k2,2n"], &input);
        assert_eq!(r.code, 0);
        let out = r.out();
        let got: Vec<&str> = out.lines().collect();
        assert_eq!(got.len(), rows.len());
        for i in 1..got.len() {
            let a: i64 = got[i - 1].split(' ').nth(1).unwrap().parse().unwrap();
            let b: i64 = got[i].split(' ').nth(1).unwrap().parse().unwrap();
            assert!(a <= b, "not sorted: {} > {}", a, b);
        }
    }
}