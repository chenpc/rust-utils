use coreutils::testutil as t;

#[test]
fn default_tab_stop_is_8() {
    let r = t::run("expand", &[], "a\tb\n");
    assert_eq!(r.out(), "a       b\n");
}

#[test]
fn tab_width_option() {
    let r = t::run("expand", &["-t", "4"], "a\tb\n");
    assert_eq!(r.out(), "a   b\n");
}

#[test]
fn explicit_tab_list() {
    let r = t::run("expand", &["-t", "3,6"], "\tx\n");
    assert_eq!(r.out(), "   x\n");
}

#[test]
fn explicit_list_second_stop() {
    let r = t::run("expand", &["-t", "3,6"], "aa\tx\n");
    assert_eq!(r.out(), "aa x\n");
}

#[test]
fn initial_only_keeps_tabs_after_nonblanks() {
    let r = t::run("expand", &["-i"], "a\tb\n");
    assert_eq!(r.out(), "a\tb\n");
}

#[test]
fn initial_only_converts_leading_tabs() {
    let r = t::run("expand", &["-i", "-t", "4"], "\ta\n");
    assert_eq!(r.out(), "    a\n");
}

#[test]
fn utf8_counts_characters() {
    let r = t::run("expand", &["-t", "4"], "é\tb\n");
    assert_eq!(r.out(), "é   b\n");
}

#[test]
fn no_utf8_counts_bytes() {
    let r = t::run("expand", &["-t", "4", "-U"], "é\tb\n");
    assert_eq!(r.out(), "é  b\n");
}

#[test]
fn line_without_trailing_newline() {
    let r = t::run("expand", &["-t", "2"], "a\tb");
    assert_eq!(r.out(), "a b");
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = t::run("expand", &["/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("expand: /nonexistent/x"));
}

#[test]
fn help_and_version() {
    let r = t::run("expand", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("expand"));
    let v = t::run("expand", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("expand"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("expand", &["-t", "2"], "a\tb\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "a b\n");
}

#[test]
fn write_error_returns_1() {
    let mut w = t::FailWriter::new(1);
    let mut e = Vec::new();
    let code = t::run_with("expand", &["-t", "4"], b"a\tb\n", &mut w, &mut e);
    assert_eq!(code, 1);
}

#[test]
fn fuzz_no_tabs_remain_when_not_initial() {
    let mut rng = t::Rng::new(0xE7);
    for _ in 0..100 {
        let parts: Vec<String> = (0..1 + rng.below(5))
            .map(|_| {
                let len = rng.below(6);
                (0..len)
                    .map(|_| char::from_u32(0x21 + rng.below(90) as u32).unwrap())
                    .collect()
            })
            .collect();
        let line = parts.join("\t") + "\n";
        let r = t::run("expand", &[], &line);
        assert_eq!(r.code, 0);
        assert!(!r.out().contains('\t'), "line={line} out={}", r.out());
    }
}

#[test]
fn fuzz_column_alignment_is_preserved() {
    let mut rng = t::Rng::new(0xE8);
    for _ in 0..100 {
        let width = 1 + rng.below(12) as usize;
        let cols = 1 + rng.below(4) as usize;
        let mut line = String::new();
        let mut expected = String::new();
        let mut col = 0usize;
        for c in 0..cols {
            let len = rng.below(4) as usize;
            let s: String = (0..len)
                .map(|_| char::from_u32(0x21 + rng.below(90) as u32).unwrap())
                .collect();
            line.push_str(&s);
            expected.push_str(&s);
            col += s.chars().count();
            if c + 1 < cols {
                line.push('\t');
                let stop = ((col / width) + 1) * width;
                expected.push_str(&" ".repeat(stop - col));
                col = stop;
            }
        }
        line.push('\n');
        expected.push('\n');
        let r = t::run("expand", &["-t", &width.to_string()], &line);
        assert_eq!(r.code, 0);
        assert_eq!(r.out(), expected, "line={line} width={width}");
    }
}
