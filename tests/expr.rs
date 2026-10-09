use coreutils::testutil as t;

fn r(args: &[&str]) -> t::Out {
    t::run("expr", args, "")
}

#[test]
fn arithmetic_sum() {
    assert_eq!(r(&["5", "+", "7"]).out(), "12\n");
    assert_eq!(r(&["10", "-", "3"]).out(), "7\n");
    assert_eq!(r(&["6", "*", "7"]).out(), "42\n");
    assert_eq!(r(&["9", "/", "2"]).out(), "4\n");
    assert_eq!(r(&["9", "%", "2"]).out(), "1\n");
}

#[test]
fn precedence_multiplication_before_addition() {
    assert_eq!(r(&["2", "+", "3", "*", "4"]).out(), "14\n");
    assert_eq!(r(&["(", "2", "+", "3", ")", "*", "4"]).out(), "20\n");
}

#[test]
fn comparisons_are_arithmetic_when_both_numeric() {
    assert_eq!(r(&["5", "<", "7"]).out(), "1\n");
    assert_eq!(r(&["5", "<", "7"]).code, 0);
    assert_eq!(r(&["7", "<", "5"]).out(), "0\n");
    assert_eq!(r(&["7", "<", "5"]).code, 1);
    assert_eq!(r(&["5", "=", "5"]).out(), "1\n");
    assert_eq!(r(&["5", "!=", "6"]).out(), "1\n");
    assert_eq!(r(&["9", ">=", "9"]).out(), "1\n");
    assert_eq!(r(&["9", "<=", "8"]).out(), "0\n");
}

#[test]
fn comparisons_are_lexicographic_when_not_both_numeric() {
    assert_eq!(r(&["abc", "<", "abd"]).out(), "1\n");
    assert_eq!(r(&["9", "<", "10"]).out(), "1\n");
    assert_eq!(r(&["a", "<", "1"]).out(), "0\n");
}

#[test]
fn logical_or_and() {
    assert_eq!(r(&["0", "|", "5"]).out(), "5\n");
    assert_eq!(r(&["3", "|", "5"]).out(), "3\n");
    assert_eq!(r(&["0", "|", "0"]).out(), "0\n");
    assert_eq!(r(&["0", "|", "0"]).code, 1);
    assert_eq!(r(&["3", "&", "5"]).out(), "3\n");
    assert_eq!(r(&["3", "&", "0"]).out(), "0\n");
}

#[test]
fn string_zero_is_falsy() {
    assert_eq!(r(&["0"]).code, 1);
    assert_eq!(r(&["x"]).code, 0);
    assert_eq!(r(&[""]).code, 1);
}

#[test]
fn anchored_pattern_match() {
    assert_eq!(r(&["hello", ":", "h.*"]).out(), "5\n");
    assert_eq!(r(&["hello", ":", "x"]).out(), "0\n");
    assert_eq!(r(&["hello", ":", "x"]).code, 1);
    assert_eq!(r(&["hello", ":", "h\\(e\\)llo"]).out(), "e\n");
    assert_eq!(r(&["match", "hello", "h.*"]).out(), "5\n");
}

#[test]
fn string_functions() {
    assert_eq!(r(&["length", "hello"]).out(), "5\n");
    assert_eq!(r(&["substr", "hello", "2", "3"]).out(), "ell\n");
    assert_eq!(r(&["substr", "hello", "0", "3"]).out(), "0\n");
    assert_eq!(r(&["index", "hello", "l"]).out(), "3\n");
    assert_eq!(r(&["index", "hello", "z"]).out(), "0\n");
}

#[test]
fn plus_forces_token_to_be_a_string() {
    assert_eq!(r(&["+", "match"]).out(), "match\n");
    assert_eq!(r(&["+", "/"]).out(), "/\n");
}

#[test]
fn syntax_error_exits_2() {
    let r0 = r(&["5", "+"]);
    assert_eq!(r0.code, 2);
    assert!(r0.err().contains("expr"));
    let r1 = r(&["(", "5"]);
    assert_eq!(r1.code, 2);
}

#[test]
fn division_by_zero_exits_2() {
    let r0 = r(&["1", "/", "0"]);
    assert_eq!(r0.code, 2);
    assert!(r0.err().contains("division by zero"));
}

#[test]
fn missing_expression_exits_2() {
    assert_eq!(r(&[]).code, 2);
}

#[test]
fn help_and_version() {
    let h = r(&["--help"]);
    assert_eq!(h.code, 0);
    assert!(h.out().contains("expr"));
    let v = r(&["--version"]);
    assert_eq!(v.code, 0);
    assert!(v.out().contains("expr"));
}

#[test]
fn e2e_binary_dispatch() {
    let o = t::run_bin("expr", &["2", "+", "3"], "");
    assert_eq!(o.out(), "5\n");
    assert_eq!(o.code, 0);
}

#[test]
fn fuzz_length_and_index_are_consistent() {
    let mut rng = t::Rng::new(0xBEEF);
    for _ in 0..150 {
        let n = 1 + rng.below(20) as usize;
        let s = String::from_utf8_lossy(&rng.bytes(n)).replace('\n', "x");
        let l = r(&["length", &s]).out().trim().parse::<usize>().unwrap();
        assert_eq!(l, s.chars().count());
        let c = String::from_utf8_lossy(&rng.bytes(1)).replace('\n', "y");
        let idx = r(&["index", &s, &c]).out().trim().parse::<usize>().unwrap();
        let want = s.chars().position(|ch| ch.to_string() == c).map(|i| i + 1).unwrap_or(0);
        assert_eq!(idx, want, "index {s} {c}");
    }
}

#[test]
fn fuzz_arithmetic_roundtrip() {
    let mut rng = t::Rng::new(0xC0FFEE);
    for _ in 0..150 {
        let a = rng.below(1000) as i64;
        let b = 1 + rng.below(100) as i64;
        let a_s = a.to_string();
        let b_s = b.to_string();
        let sum = r(&[&a_s, "+", &b_s]).out().trim().parse::<i64>().unwrap();
        assert_eq!(sum, a + b);
        let back = r(&[&sum.to_string(), "-", &b_s]).out().trim().parse::<i64>().unwrap();
        assert_eq!(back, a, "roundtrip {a} + {b}");
        let mul = r(&[&a_s, "*", &b_s]).out().trim().parse::<i64>().unwrap();
        assert_eq!(mul, a * b);
        let q = r(&[&mul.to_string(), "/", &b_s]).out().trim().parse::<i64>().unwrap();
        assert_eq!(q, a, "division roundtrip");
    }
}

#[test]
fn fuzz_or_returns_first_truthy() {
    let mut rng = t::Rng::new(0x5EED);
    for _ in 0..100 {
        let a = rng.below(3).to_string();
        let b = rng.below(3).to_string();
        let got = r(&[&a, "|", &b]).out().trim().to_string();
        let want = if a != "0" { a.clone() } else { b.clone() };
        assert_eq!(got, want, "or {a} | {b}");
    }
}
