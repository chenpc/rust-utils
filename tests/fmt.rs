use coreutils::testutil as t;

fn fmt(args: &[&str], input: &str) -> t::Out {
    t::run("fmt", args, input)
}

#[test]
fn reflows_to_width() {
    let r = fmt(&["-w", "20"], "hello world this is a test of split only behaviour\n");
    assert_eq!(r.code, 0);
    let out = r.out();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "hello world this is");
    assert_eq!(lines[1], "a test of split only");
    assert_eq!(lines[2], "behaviour");
}

#[test]
fn default_width_is_75() {
    let long = "a".repeat(200);
    let r = fmt(&[], format!("{} tail", long).as_str());
    let out = r.out();
    let lines: Vec<&str> = out.lines().collect();
    // a word longer than WIDTH cannot be broken
    assert_eq!(lines[0], "a".repeat(200));
    assert_eq!(lines[1], "tail");
}

#[test]
fn blank_line_ends_paragraph_and_passes_through() {
    let r = fmt(&["-w", "20"], "para one line\nsecond line\n\nnext para\n");
    assert_eq!(r.out(), "para one line second
line

next para
");
}

#[test]
fn no_extra_blank_line_for_trailing_newline() {
    assert_eq!(fmt(&["-w", "20"], "a b\n").out(), "a b\n");
}

#[test]
fn split_only_does_not_merge_lines() {
    let r = fmt(&["-w", "10", "-s"], "aa bb cc dd\n ee ff\n");
    assert_eq!(r.out(), "aa bb cc\ndd\n ee ff\n");
}

#[test]
fn uniform_spacing_two_spaces_after_sentence() {
    let r = fmt(&["-w", "40", "-u"], "one.  two three\nfour.\n");
    assert_eq!(r.out(), "one.  two three four.\n");
}

#[test]
fn crown_margin_keeps_first_indent_and_uses_second_line_indent() {
    let r = fmt(&["-w", "20", "-c"], "  first line here\n    second line here\n");
    assert_eq!(r.out(), "  first line here\n    second line here\n");
}

#[test]
fn tagged_paragraph_splits_equal_indents() {
    let r = fmt(&["-w", "20", "-t"], "  same indent\n  other line\n");
    assert_eq!(r.out(), "  same indent\n  other line\n");
}

#[test]
fn preserve_headers_keeps_header_line() {
    let r = fmt(&["-w", "20", "-m"], "Subject: test\nbody text here\n");
    assert_eq!(r.out(), "Subject: test\nbody text here\n");
}

#[test]
fn prefix_reformats_only_matching_lines() {
    let r = fmt(&["-w", "10", "-p", "> "], "> a b c d e\nplain line\n");
    assert_eq!(r.out(), "> a b c d\n> e\nplain line\n");
}

#[test]
fn skip_prefix_leaves_lines_untouched() {
    let r = fmt(
        &["-w", "12", "-P", "# "],
        "# keep this long line\nother long line here\n",
    );
    assert_eq!(r.out(), "# keep this long line\nother long\nline here\n");
}

#[test]
fn exact_prefix_requires_no_leading_whitespace() {
    let r = fmt(
        &["-w", "12", "-p", ">", "-x"],
        "  > indented\n> exact\n",
    );
    assert_eq!(r.out(), "  > indented\n> exact\n");
}

#[test]
fn tab_width_counts_for_length_only() {
    let r = fmt(&["-w", "10", "-T", "4"], "aa\tbb cc\n");
    // "aa\tbb" counts as 2+4+2 = 8, adding " cc" would be 11 > 10
    assert_eq!(r.out(), "aa\tbb\ncc\n");
}

#[test]
fn quick_breaks_at_goal() {
    let r = fmt(&["-w", "20", "-g", "10", "-q"], "aa bb cc dd ee ff\n");
    assert_eq!(r.out(), "aa bb cc dd\nee ff\n");
}

#[test]
fn goal_greater_than_width_is_error() {
    let r = fmt(&["-w", "10", "-g", "20"], "x\n");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("goal"));
}

#[test]
fn missing_operand_and_unknown_option_exit_2() {
    assert_eq!(fmt(&["-p"], "x\n").code, 2);
    assert_eq!(fmt(&["-Z"], "x\n").code, 2);
    assert_eq!(fmt(&["-w", "abc"], "x\n").code, 2);
}

#[test]
fn missing_file_exit_1() {
    let r = fmt(&["/definitely/not/here"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("fmt:"));
}

#[test]
fn help_and_version() {
    let r = fmt(&["-h"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("fmt"));
    let v = fmt(&["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.out().contains("fmt"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("fmt", &["-w", "10"], "aa bb cc dd\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "aa bb cc\ndd\n");
}

#[test]
fn fuzz_word_order_is_preserved() {
    let mut rng = t::Rng::new(0xF00D);
    for _ in 0..100 {
        let n = 3 + rng.below(20);
        let mut words = Vec::new();
        for _ in 0..n {
            let len = 1 + rng.below(6);
            let mut w = String::new();
            for _ in 0..len {
                w.push((0x61 + rng.below(20)) as u8 as char);
            }
            words.push(w);
        }
        let input = words.join(" ") + "\n";
        let width = (10 + rng.below(30)) as usize;
        let wstr = width.to_string();
        let r = fmt(&["-w", &wstr], &input);
        assert_eq!(r.code, 0);
        let out = r.out();
        let got: Vec<&str> = out.split_whitespace().collect();
        assert_eq!(got, words.iter().map(|s| s.as_str()).collect::<Vec<&str>>());
        let out = r.out();
        for line in out.lines() {
            assert!(line.len() <= width || line.split_whitespace().count() == 1);
        }
    }
}

#[test]
fn fuzz_split_only_keeps_line_count() {
    let mut rng = t::Rng::new(0xBEEF);
    for _ in 0..60 {
        let lines: Vec<String> = (0..3 + rng.below(4))
            .map(|_| {
                let n = 1 + rng.below(10);
                (0..n)
                    .map(|_| {
                        let len = 1 + rng.below(5);
                        (0..len).map(|_| (0x61 + rng.below(20)) as u8 as char).collect::<String>()
                    })
                    .collect::<Vec<String>>()
                    .join(" ")
            })
            .collect();
        let input = lines.join("\n") + "\n";
        let width = (12 + rng.below(20)) as usize;
        let wstr = width.to_string();
        let r = fmt(&["-w", &wstr, "-s"], &input);
        assert_eq!(r.code, 0);
        let out = r.out();
        let out_lines: Vec<&str> = out.lines().collect();
        assert!(out_lines.len() >= lines.len());
        let out = r.out();
        let got: Vec<&str> = out.split_whitespace().collect();
        let mut want: Vec<String> = Vec::new();
        for l in &lines {
            for w in l.split_whitespace() {
                want.push(w.to_string());
            }
        }
        assert_eq!(got, want);
    }
}