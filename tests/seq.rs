use coreutils::testutil as t;

#[test]
fn single_number_counts_from_one() {
    let r = t::run("seq", &["5"], "");
    assert_eq!(r.out(), "1\n2\n3\n4\n5\n");
}

#[test]
fn two_numbers_are_first_and_last() {
    let r = t::run("seq", &["2", "5"], "");
    assert_eq!(r.out(), "2\n3\n4\n5\n");
}

#[test]
fn three_numbers_are_first_increment_last() {
    let r = t::run("seq", &["1", "2", "10"], "");
    assert_eq!(r.out(), "1\n3\n5\n7\n9\n");
}

#[test]
fn descending_with_negative_increment() {
    let r = t::run("seq", &["5", "-1", "1"], "");
    assert_eq!(r.out(), "5\n4\n3\n2\n1\n");
}

#[test]
fn float_increment() {
    let r = t::run("seq", &["0", "0.5", "1.5"], "");
    assert_eq!(r.out(), "0\n0.5\n1\n1.5\n");
}

#[test]
fn separator_and_terminator() {
    let r = t::run("seq", &["-s", ",", "-t", "!", "1", "3"], "");
    assert_eq!(r.out(), "1,2,3!");
}

#[test]
fn tab_escape_in_separator() {
    let r = t::run("seq", &["--separator=\\t", "1", "3"], "");
    assert_eq!(r.out(), "1\t2\t3\n");
}

#[test]
fn equal_width_pads_with_zeros() {
    let r = t::run("seq", &["-w", "1", "10"], "");
    assert_eq!(r.out(), "01\n02\n03\n04\n05\n06\n07\n08\n09\n10\n");
}

#[test]
fn equal_width_auto_uses_widest() {
    let r = t::run("seq", &["--equal-width", "9", "11"], "");
    assert_eq!(r.out(), "09\n10\n11\n");
}

#[test]
fn format_percent_g() {
    let r = t::run("seq", &["-f", "%.1f", "1", "3"], "");
    assert_eq!(r.out(), "1.0\n2.0\n3.0\n");
}

#[test]
fn format_percent_d() {
    let r = t::run("seq", &["--format=%d", "1", "2"], "");
    assert_eq!(r.out(), "1\n2\n");
}

#[test]
fn missing_operand_is_exit_2() {
    let r = t::run("seq", &[], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("missing operand"));
}

#[test]
fn zero_increment_is_error() {
    let r = t::run("seq", &["1", "0", "5"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid increment"));
}

#[test]
fn unknown_option_is_exit_2() {
    let r = t::run("seq", &["-x", "5"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("unknown option"));
}

#[test]
fn help_and_version() {
    let r = t::run("seq", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("seq"));
    let v = t::run("seq", &["-V"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("seq"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("seq", &["1", "3"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "1\n2\n3\n");
}

#[test]
fn fuzz_count_matches_range() {
    let mut rng = t::Rng::new(0xBEEF);
    for _ in 0..100 {
        let first = rng.below(100) as i64;
        let incr = 1 + rng.below(9) as i64;
        let last = first + incr * (1 + rng.below(20) as i64);
        let strs = [first.to_string(), incr.to_string(), last.to_string()];
        let a: Vec<&str> = strs.iter().map(|x| x.as_str()).collect();
        let r = t::run("seq", &a, "");
        assert_eq!(r.code, 0);
        let s = r.out();
        let lines: Vec<&str> = s.lines().collect();
        // invariant: number of emitted values == floor((last-first)/incr)+1
        let expected = ((last - first) / incr + 1) as usize;
        assert_eq!(lines.len(), expected, "range {}..{} step {}", first, last, incr);
        // invariant: values are an arithmetic progression
        for (i, l) in lines.iter().enumerate() {
            assert_eq!(l.parse::<i64>().unwrap(), first + incr * i as i64);
        }
    }
}

#[test]
fn fuzz_separator_preserved() {
    let mut rng = t::Rng::new(0xC0FFEE);
    for _ in 0..50 {
        let sep = char::from_u32(0x21 + rng.below(90) as u32).unwrap();
        if sep.is_ascii_digit() || sep == '-' || sep == ' ' { continue; }
        let n = 1 + rng.below(10);
        let strs = vec!["-s".to_string(), sep.to_string(), "1".to_string(), n.to_string()];
        let a: Vec<&str> = strs.iter().map(|x| x.as_str()).collect();
        let r = t::run("seq", &a, "");
        assert_eq!(r.code, 0);
        let o = r.out();
        let parts: Vec<&str> = o.trim_end().split(sep).collect();
        assert_eq!(parts.len(), n as usize, "separator {} splits into {} parts", sep, n);
    }
}
