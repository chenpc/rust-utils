use coreutils::testutil as t;

fn p(args: &[&str]) -> t::Out {
    t::run("printf", args, "")
}

#[test]
fn plain_format_prints_once() {
    assert_eq!(p(&["hello"]).out(), "hello");
    assert_eq!(p(&["hello", "x"]).out(), "hello");
}

#[test]
fn escapes_in_format() {
    assert_eq!(p(&["a\\nb"]).out(), "a\nb");
    assert_eq!(p(&["a\\tb"]).out(), "a\tb");
    assert_eq!(p(&["a\\\\b"]).out(), "a\\b");
    assert_eq!(p(&["\\x41"]).out(), "A");
    assert_eq!(p(&["\\101"]).out(), "A");
    assert_eq!(p(&["\\u00e9"]).out(), "é");
    assert_eq!(p(&["\\x"]).out(), "\\x");
    assert_eq!(p(&["\\q"]).out(), "\\q");
}

#[test]
fn c_stops_output() {
    assert_eq!(p(&["a\\cb"]).out(), "a");
    assert_eq!(p(&["\\c"]).out(), "");
}

#[test]
fn percent_percent() {
    assert_eq!(p(&["100%%"]).out(), "100%");
}

#[test]
fn string_conversion() {
    assert_eq!(p(&["%s", "abc"]).out(), "abc");
    assert_eq!(p(&["%s%s", "a", "b"]).out(), "ab");
    assert_eq!(p(&["%.2s", "abcde"]).out(), "ab");
    assert_eq!(p(&["%4s", "a"]).out(), "   a");
    assert_eq!(p(&["%-4s|", "a"]).out(), "a   |");
}

#[test]
fn format_repeats_while_args_remain() {
    assert_eq!(p(&["%s\n", "a", "b", "c"]).out(), "a\nb\nc\n");
    assert_eq!(p(&["%s\n", "a", "b", "c", "d"]).out(), "a\nb\nc\nd\n");
    assert_eq!(p(&["%s\n"]).out(), "\n");
}

#[test]
fn b_expands_escapes_in_argument() {
    assert_eq!(p(&["%b", "a\\nb"]).out(), "a\nb");
    assert_eq!(p(&["%b", "x\\x41"]).out(), "xA");
}

#[test]
fn char_conversion() {
    assert_eq!(p(&["%c", "q"]).out(), "q");
    assert_eq!(p(&["%4c", "q"]).out(), "   q");
    assert_eq!(p(&["%c", "abc"]).out(), "a");
}

#[test]
fn integer_conversions() {
    assert_eq!(p(&["%d", "7"]).out(), "7");
    assert_eq!(p(&["%4.3i", "7"]).out(), " 007");
    assert_eq!(p(&["%d", "010"]).out(), "8");
    assert_eq!(p(&["%d", "0xABC"]).out(), "2748");
    assert_eq!(p(&["%d", "'a"]).out(), "97");
    assert_eq!(p(&["%x", "255"]).out(), "ff");
    assert_eq!(p(&["%X", "255"]).out(), "FF");
    assert_eq!(p(&["%o", "8"]).out(), "10");
    assert_eq!(p(&["%+d", "5"]).out(), "+5");
    assert_eq!(p(&["% d", "5"]).out(), " 5");
    assert_eq!(p(&["%#x", "255"]).out(), "0xff");
    assert_eq!(p(&["%05d", "42"]).out(), "00042");
    assert_eq!(p(&["%d", "-3"]).out(), "-3");
}

#[test]
fn star_width_and_precision() {
    assert_eq!(p(&["%*d", "5", "42"]).out(), "   42");
    assert_eq!(p(&["%.*f", "2", "3.14159"]).out(), "3.14");
    assert_eq!(p(&["%*.*f", "6", "2", "1.5"]).out(), "  1.50");
}

#[test]
fn float_conversions() {
    assert_eq!(p(&["%f", "1.5"]).out(), "1.500000");
    assert_eq!(p(&["%.2f", "1.5"]).out(), "1.50");
    assert_eq!(p(&["%e", "1.5"]).out(), "1.500000e+00");
    assert_eq!(p(&["%E", "1.5"]).out(), "1.500000E+00");
    assert_eq!(p(&["%g", "1.5"]).out(), "1.5");
    assert_eq!(p(&["%g", "0"]).out(), "0");
    assert_eq!(p(&["%.3g", "123456"]).out(), "1.23e+05");
}

#[test]
fn shell_quote() {
    assert_eq!(p(&["%q", "abc"]).out(), "abc");
    assert_eq!(p(&["%q", "a b"]).out(), "$'a b'");
    assert_eq!(p(&["%q", "a'b"]).out(), "$'a\\'b'");
    assert_eq!(p(&["%q", ""]).out(), "''");
}

#[test]
fn invalid_conversion_exits_1() {
    let o = p(&["%z", "1"]);
    assert_eq!(o.code, 1);
    assert!(o.err().contains("invalid conversion"));
    let o2 = p(&["%"]);
    assert_eq!(o2.code, 1);
}

#[test]
fn invalid_number_exits_1() {
    let o = p(&["%d", "abc"]);
    assert_eq!(o.code, 1);
    assert!(o.err().contains("invalid number"));
}

#[test]
fn write_error_returns_1() {
    let mut w = t::FailWriter::new(3);
    let mut e = t::FailWriter::new(1000);
    let code = t::run_with("printf", &["%s", "abcdefgh"], &[], &mut w, &mut e);
    assert_eq!(code, 1);
    assert_eq!(w.written, 3);
}

#[test]
fn help_and_version() {
    let h = p(&["--help"]);
    assert_eq!(h.code, 0);
    assert!(h.out().contains("printf"));
    let v = p(&["-V"]);
    assert_eq!(v.code, 0);
    assert!(v.out().contains("printf"));
}

#[test]
fn e2e_binary_dispatch() {
    let o = t::run_bin("printf", &["%s\n", "hi"], "");
    assert_eq!(o.code, 0);
    assert_eq!(o.out(), "hi\n");
}

#[test]
fn fuzz_string_identity() {
    let mut rng = t::Rng::new(0x5EED);
    for _ in 0..200 {
        let n = 1 + rng.below(4) as usize;
        let mut args = vec!["%s|".to_string()];
        let mut want = String::new();
        for _ in 0..n {
            let len = 1 + rng.below(8) as usize;
            let s: String = (0..len)
                .map(|_| (0x21 + rng.below(90) as u8) as char)
                .collect();
            args.push(s.clone());
            want.push_str(&s);
            want.push('|');
        }
        let strs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let got = p(&strs);
        assert_eq!(got.code, 0);
        assert_eq!(got.out(), want);
    }
}

#[test]
fn fuzz_integer_roundtrip() {
    let mut rng = t::Rng::new(0x27);
    for _ in 0..200 {
        let v = rng.below(100000) as i64;
        let s = v.to_string();
        let args = vec!["%d\n", s.as_str()];
        let got = p(&args);
        assert_eq!(got.code, 0);
        assert_eq!(got.out(), format!("{}\n", v));
        // 十六進與八進必須是同一數值的另一進位表示
        let hx = p(&["%x", &s]);
        assert_eq!(u64::from_str_radix(&hx.out(), 16).unwrap(), v as u64);
        let oc = p(&["%o", &s]);
        assert_eq!(u64::from_str_radix(&oc.out(), 8).unwrap(), v as u64);
    }
}

#[test]
fn fuzz_width_padding() {
    let mut rng = t::Rng::new(0x99);
    for _ in 0..150 {
        let w = 1 + rng.below(12) as usize;
        let s: String = (0..(1 + rng.below(6) as usize))
            .map(|_| (0x21 + rng.below(90) as u8) as char)
            .collect();
        let ws = w.to_string();
        let args = vec!["%*s", ws.as_str(), s.as_str()];
        let got = p(&args);
        assert_eq!(got.code, 0);
        if s.len() >= w {
            assert_eq!(got.out(), s);
        } else {
            assert_eq!(got.out().len(), w);
            assert!(got.out().ends_with(&s));
        }
    }
}
