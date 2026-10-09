use coreutils::testutil as t;

#[test]
fn bytes_basic() {
    let r = t::run("cut", &["-b", "1,3"], "abcde\n");
    assert_eq!(r.out(), "ac\n");
    assert_eq!(r.code, 0);
}

#[test]
fn bytes_range() {
    let r = t::run("cut", &["-b", "2-4"], "abcde\n");
    assert_eq!(r.out(), "bcd\n");
}

#[test]
fn bytes_open_range() {
    let r = t::run("cut", &["-b", "3-"], "abcde\n");
    assert_eq!(r.out(), "cde\n");
}

#[test]
fn bytes_leading_range() {
    let r = t::run("cut", &["-b", "1-2"], "abcde\n");
    assert_eq!(r.out(), "ab\n");
}

#[test]
fn list_starting_with_dash_is_an_option_error() {
    let r = t::run("cut", &["-b", "-2"], "abcde\n");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("unknown option"));
}

#[test]
fn characters_count_multibyte_as_one() {
    let r = t::run("cut", &["-c", "1,2"], "aébc\n");
    assert_eq!(r.out(), "aé\n");
}

#[test]
fn bytes_partial_multibyte_without_n() {
    // -b selects raw bytes; a continuation byte alone is emitted
    let r = t::run("cut", &["-b", "2"], "aé\n");
    assert_eq!(r.out_bytes(), vec![0xC3, 0x0A]);
}

#[test]
fn bytes_no_partial_keeps_whole_char() {
    let r = t::run("cut", &["-b", "2", "-n"], "aé\n");
    assert_eq!(r.out(), "é\n");
}

#[test]
fn fields_default_tab() {
    let r = t::run("cut", &["-f", "2"], "a\tb\tc\n");
    assert_eq!(r.out(), "b\n");
}

#[test]
fn fields_custom_delimiter() {
    let r = t::run("cut", &["-d", ",", "-f", "1,3"], "a,b,c\n");
    assert_eq!(r.out(), "a,c\n");
}

#[test]
fn missing_field_is_empty() {
    let r = t::run("cut", &["-f", "3"], "a\tb\n");
    assert_eq!(r.out(), "\n");
}

#[test]
fn only_delimited_skips_lines_without_delimiter() {
    let r = t::run("cut", &["-f", "2", "-s"], "a\tb\nc\n");
    assert_eq!(r.out(), "b\n");
}

#[test]
fn whitespace_fields() {
    let r = t::run("cut", &["-f", "2", "-w"], "a  b c\n");
    assert_eq!(r.out(), "b\n");
}

#[test]
fn complement_bytes() {
    let r = t::run("cut", &["-b", "1", "--complement"], "abc\n");
    assert_eq!(r.out(), "bc\n");
}

#[test]
fn complement_fields() {
    let r = t::run("cut", &["-f", "2", "--complement"], "a\tb\tc\n");
    assert_eq!(r.out(), "a\tc\n");
}

#[test]
fn output_delimiter() {
    let r = t::run("cut", &["-f", "1,3", "-O", "+"], "a\tb\tc\n");
    assert_eq!(r.out(), "a+c\n");
}

#[test]
fn zero_terminated() {
    let r = t::run_bytes("cut", &["-z", "-f", "2"], b"a\tb\0c\td\0");
    assert_eq!(r.out_bytes(), b"b\0d\0");
}

#[test]
fn no_mode_is_exit_2() {
    let r = t::run("cut", &["1,2"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("must specify a list"));
}

#[test]
fn empty_list_is_exit_2() {
    let r = t::run("cut", &["-b", ""], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("must specify a list"));
}

#[test]
fn invalid_range_is_exit_2() {
    let r = t::run("cut", &["-b", "5-3"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("invalid range"));
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = t::run("cut", &["-b", "1", "/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("cut: /nonexistent/x"));
}

#[test]
fn help_and_version() {
    let r = t::run("cut", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("cut"));
    let v = t::run("cut", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("cut"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("cut", &["-f", "2"], "a\tb\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "b\n");
}

#[test]
fn write_error_returns_1() {
    let mut w = t::FailWriter::new(2);
    let mut e = Vec::new();
    let code = t::run_with("cut", &["-b", "1,2,3"], b"abc\nabc\n", &mut w, &mut e);
    assert_eq!(code, 1);
}

#[test]
fn fuzz_bytes_selection() {
    let mut rng = t::Rng::new(0xC0FFEE);
    for _ in 0..120 {
        let len = 1 + rng.below(20) as usize;
        let mut data = rng.bytes(len);
        for b in data.iter_mut() {
            if *b == 0x0A {
                *b = 0x20;
            }
        }
        let nsel = 1 + rng.below(4);
        let mut sel = Vec::new();
        for _ in 0..nsel {
            sel.push(1 + rng.below(len as u64));
        }
        sel.sort();
        sel.dedup();
        let list = sel.iter().map(|x| x.to_string()).collect::<Vec<_>>().join(",");
        let r = t::run_bytes("cut", &["-b", &list], &data);
        assert_eq!(r.code, 0);
        let mut expected: Vec<u8> = sel.iter().map(|p| data[*p as usize - 1]).collect();
        expected.push(b'\n');
        assert_eq!(r.out_bytes(), expected, "list={list} data={:?}", data);
    }
}

#[test]
fn fuzz_fields_selection() {
    let mut rng = t::Rng::new(0xBADF00D);
    for _ in 0..120 {
        let d = char::from_u32(0x21 + rng.below(60) as u32).unwrap();
        if d == '\\' {
            continue;
        }
        let nfields = 1 + rng.below(5);
        let mut fields = Vec::new();
        for _ in 0..nfields {
            let s: String = (0..rng.below(4))
                .map(|_| {
                    let c = loop {
                        let c = char::from_u32(0x21 + rng.below(90) as u32).unwrap();
                        if c != d && c != '\n' {
                            break c;
                        }
                    };
                    c
                })
                .collect();
            fields.push(s);
        }
        let line = fields.join(&d.to_string()) + "\n";
        let k = 1 + rng.below(nfields);
        let r = t::run("cut", &["-f", &k.to_string(), "-d", &d.to_string()], &line);
        assert_eq!(r.code, 0);
        assert_eq!(r.out(), format!("{}\n", fields[k as usize - 1]), "line={line} k={k}");
    }
}

#[test]
fn fuzz_complement_matches_full_line() {
    let mut rng = t::Rng::new(0x5EED);
    for _ in 0..100 {
        let len = 1 + rng.below(15) as usize;
        let mut data = rng.bytes(len);
        for b in data.iter_mut() {
            if *b == 0x0A {
                *b = 0x20;
            }
        }
        let p = 1 + rng.below(len as u64);
        let r = t::run_bytes("cut", &["-b", &p.to_string(), "--complement"], &data);
        assert_eq!(r.code, 0);
        let mut expected = data.clone();
        expected.remove(p as usize - 1);
        expected.push(b'\n');
        assert_eq!(r.out_bytes(), expected, "p={p} data={:?}", data);
    }
}
