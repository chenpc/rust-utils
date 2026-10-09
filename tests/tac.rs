use coreutils::testutil as t;

#[test]
fn reverses_lines() {
    let r = t::run("tac", &[], "a\nb\nc\n");
    assert_eq!(r.out(), "c\nb\na\n");
}

#[test]
fn no_trailing_separator_still_reverses() {
    let r = t::run("tac", &[], "a\nb\nc");
    assert_eq!(r.out(), "c\nb\na\n");
}

#[test]
fn custom_separator() {
    let r = t::run("tac", &["-s", "X"], "aXbXc");
    assert_eq!(r.out(), "cXbXaX");
}

#[test]
fn before_attaches_separator_before_each_record() {
    let r = t::run("tac", &["-b", "-s", "X"], "XaXbXc");
    assert_eq!(r.out(), "XcXbXa");
}

#[test]
fn regex_separator() {
    let r = t::run("tac", &["-r", "-s", "[0-9]+"], "a1b22c333d9");
    assert_eq!(r.out(), "d9c333b22a1");
}

#[test]
fn regex_alternation() {
    let r = t::run("tac", &["--regex", "--separator", "a|b"], "xaybzc");
    assert_eq!(r.out(), "zcybxa");
}

#[test]
fn regex_star_and_escape() {
    let r = t::run("tac", &["-r", "-s", "x*"], "abxxcd");
    assert_eq!(r.out(), "cdabxx");
}

#[test]
fn empty_input_produces_nothing() {
    let r = t::run("tac", &[], "");
    assert_eq!(r.out(), "");
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = t::run("tac", &["/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("tac: /nonexistent/x"));
}

#[test]
fn dash_reads_stdin() {
    let r = t::run("tac", &["-"], "a\nb\n");
    assert_eq!(r.out(), "b\na\n");
}

#[test]
fn help_and_version() {
    let r = t::run("tac", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("tac"));
    let v = t::run("tac", &["-V"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("tac"));
}

#[test]
fn unknown_option_is_exit_2() {
    let r = t::run("tac", &["-x"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("unknown option"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("tac", &[], "a\nb\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "b\na\n");
}

#[test]
fn fuzz_record_multiset_is_preserved() {
    let mut rng = t::Rng::new(0x5A7A);
    for _ in 0..100 {
        let sep = char::from_u32(0x21 + rng.below(60) as u32).unwrap();
        if sep == '\\' { continue; }
        let mut input = String::new();
        for _ in 0..rng.below(6) {
            let n = 1 + rng.below(8);
            for _ in 0..n {
                let c = loop {
                    let c = char::from_u32(0x21 + rng.below(90) as u32).unwrap();
                    if c != sep { break c; }
                };
                input.push(c);
            }
            input.push(sep);
        }
        let r = t::run("tac", &["-s", &sep.to_string()], &input);
        assert_eq!(r.code, 0);
        // 不變量：逆序輸出後，把記錄取回來必須與輸入的記錄序列相反
        let orig: Vec<&str> = input.split(sep).filter(|x| !x.is_empty()).collect();
        let o = r.out();
        let got: Vec<&str> = o.split(sep).filter(|x| !x.is_empty()).collect();
        let rev: Vec<&str> = orig.iter().rev().copied().collect();
        assert_eq!(got, rev, "sep {} input {:?}", sep, input);
    }
}
