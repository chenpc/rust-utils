use coreutils::testutil as t;

#[test]
fn default_numbers_nonblank_lines() {
    let r = t::run("nl", &[], "a\n\nb\n");
    assert_eq!(r.out(), "     1\ta\n\n     2\tb\n");
}

#[test]
fn body_style_a_numbers_blank_lines() {
    let r = t::run("nl", &["-b", "a"], "a\n\nb\n");
    assert_eq!(r.out(), "     1\ta\n     2\t\n     3\tb\n");
}

#[test]
fn body_style_n_numbers_nothing() {
    let r = t::run("nl", &["-b", "n"], "a\nb\n");
    assert_eq!(r.out(), "a\nb\n");
}

#[test]
fn body_style_p_numbers_only_matches() {
    let r = t::run("nl", &["-b", "px", "-w", "2"], "ab\ncd\nx\n");
    assert_eq!(r.out(), "ab\ncd\n 1\tx\n");
}

#[test]
fn number_format_rz_pads_zeros() {
    let r = t::run("nl", &["-n", "rz", "-w", "3"], "a\nb\n");
    assert_eq!(r.out(), "001\ta\n002\tb\n");
}

#[test]
fn number_format_ln_left_justifies() {
    let r = t::run("nl", &["-n", "ln", "-w", "3"], "a\nb\n");
    assert_eq!(r.out(), "1  \ta\n2  \tb\n");
}

#[test]
fn increment_and_start() {
    let r = t::run("nl", &["-i", "2", "-v", "10"], "a\nb\n");
    assert_eq!(r.out(), "    10\ta\n    12\tb\n");
}

#[test]
fn number_separator() {
    let r = t::run("nl", &["-s", ":", "-w", "2"], "a\nb\n");
    assert_eq!(r.out(), " 1:a\n 2:b\n");
}

#[test]
fn section_delimiter_resets_numbering() {
    let r = t::run("nl", &["-w", "2"], "a\n\\:\nb\n");
    assert_eq!(r.out(), " 1\ta\n\\:\n 1\tb\n");
}

#[test]
fn custom_delimiter() {
    let r = t::run("nl", &["-d", "X", "-w", "2"], "a\nX\nb\n");
    assert_eq!(r.out(), " 1\ta\nX\n 1\tb\n");
}

#[test]
fn no_renumber_keeps_counter() {
    let r = t::run("nl", &["-p", "-w", "2"], "a\n\\:\nb\n");
    assert_eq!(r.out(), " 1\ta\n\\:\n 2\tb\n");
}

#[test]
fn join_blank_lines_groups() {
    let r = t::run("nl", &["-b", "a", "-l", "2", "-w", "2"], "a\n\n\nb\n");
    assert_eq!(r.out(), " 1\ta\n  \t\n 2\t\n 3\tb\n");
}

#[test]
fn long_options_are_accepted() {
    let r = t::run(
        "nl",
        &[
            "--body-numbering",
            "a",
            "--number-width",
            "2",
            "--number-separator",
            ":",
            "--starting-line-number",
            "5",
            "--line-increment",
            "3",
            "--number-format",
            "ln",
        ],
        "a\nb\n",
    );
    assert_eq!(r.out(), "5 :a\n8 :b\n");
}

#[test]
fn long_option_missing_argument_is_exit_2() {
    let r = t::run("nl", &["--number-width"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("missing argument"));
}

#[test]
fn short_option_missing_argument_is_exit_2() {
    let r = t::run("nl", &["-w"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("missing argument"));
}

#[test]
fn number_wider_than_width_is_not_truncated() {
    let r = t::run("nl", &["-w", "1", "-v", "10"], "a\nb\n");
    assert_eq!(r.out(), "10\ta\n11\tb\n");
}

#[test]
fn invalid_format_is_exit_2() {
    let r = t::run("nl", &["-n", "zz"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("invalid number format"));
}

#[test]
fn invalid_style_is_exit_2() {
    let r = t::run("nl", &["-b", "z"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("invalid numbering style"));
}

#[test]
fn missing_file_is_error_and_exit_1() {
    let r = t::run("nl", &["/nonexistent/x"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("nl: /nonexistent/x"));
}

#[test]
fn help_and_version() {
    let r = t::run("nl", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("nl"));
    let v = t::run("nl", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.err().contains("nl"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("nl", &["-w", "2"], "a\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), " 1\ta\n");
}

#[test]
fn write_error_returns_1() {
    let mut w = t::FailWriter::new(2);
    let mut e = Vec::new();
    let code = t::run_with("nl", &[], b"a\nb\nc\n", &mut w, &mut e);
    assert_eq!(code, 1);
}

#[test]
fn fuzz_numbering_sequence() {
    let mut rng = t::Rng::new(0xAE00);
    for _ in 0..100 {
        let n = 1 + rng.below(12);
        let mut lines = Vec::new();
        for _ in 0..n {
            if rng.below(4) == 0 {
                lines.push(String::new());
            } else {
                let s: String = (0..rng.below(4))
                    .map(|_| char::from_u32(0x21 + rng.below(90) as u32).unwrap())
                    .collect();
                lines.push(s);
            }
        }
        let input = lines.join("\n") + "\n";
        let r = t::run("nl", &["-b", "a", "-w", "6"], &input);
        assert_eq!(r.code, 0);
        let o = r.out();
        let got: Vec<&str> = o.lines().collect();
        assert_eq!(got.len(), lines.len(), "input={input}");
        for (i, g) in got.iter().enumerate() {
            let num = g.split('\t').next().unwrap().trim();
            assert_eq!(num, (i + 1).to_string(), "line {i} of {input}");
        }
    }
}
