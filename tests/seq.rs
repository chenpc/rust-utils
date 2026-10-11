use coreutils::testutil as t;

// Expectations below were taken from the reference `seq` (uutils coreutils 0.10.0)
// and cross-checked against this implementation (pair-diff battery, 376 cases).

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
fn float_increment_keeps_decimal_places() {
    let r = t::run("seq", &["0", "0.5", "1.5"], "");
    assert_eq!(r.out(), "0.0\n0.5\n1.0\n1.5\n");
    let r = t::run("seq", &["1", "0.25", "2"], "");
    assert_eq!(r.out(), "1.00\n1.25\n1.50\n1.75\n2.00\n");
}

#[test]
fn decimal_count_is_exact() {
    // floor((LAST-FIRST)/INCREMENT) in exact decimal arithmetic
    let r = t::run("seq", &["0", "0.1", "0.3"], "");
    assert_eq!(r.out(), "0.0\n0.1\n0.2\n0.3\n");
    let r = t::run("seq", &["1", "3", "12"], "");
    assert_eq!(r.out(), "1\n4\n7\n10\n");
    let r = t::run("seq", &["1", "3", "11"], "");
    assert_eq!(r.out(), "1\n4\n7\n10\n");
    let r = t::run("seq", &["0", "7", "20"], "");
    assert_eq!(r.out(), "0\n7\n14\n");
}

#[test]
fn scientific_notation_literals_expand() {
    let r = t::run("seq", &["1e3", "1e3", "3e3"], "");
    assert_eq!(r.out(), "1000\n2000\n3000\n");
    let r = t::run("seq", &["1e-3", "1e-3", "3e-3"], "");
    assert_eq!(r.out(), "0.001\n0.002\n0.003\n");
}

#[test]
fn very_long_literals_use_big_decimal() {
    let r = t::run("seq", &["1e100", "1e100"], "");
    assert_eq!(r.out(), "1".to_string() + &"0".repeat(100) + "\n");
    let r = t::run("seq", &["1e-100", "1e-100"], "");
    assert_eq!(r.out(), format!("0.{}1\n", "0".repeat(99)));
    let r = t::run("seq", &["1", "1e-100", "1"], "");
    assert_eq!(r.out(), format!("1.{}\n", "0".repeat(100)));
}

#[test]
fn negative_zero_first_keeps_its_sign() {
    let r = t::run("seq", &["-0", "3"], "");
    assert_eq!(r.out(), "-0\n1\n2\n3\n");
    let r = t::run("seq", &["--", "-0", "3"], "");
    assert_eq!(r.out(), "-0\n1\n2\n3\n");
    let r = t::run("seq", &["-0.5", "0.5", "0.5"], "");
    assert_eq!(r.out(), "-0.5\n0.0\n0.5\n");
}

#[test]
fn separator_and_terminator() {
    let r = t::run("seq", &["-s", ",", "-t", "!", "1", "3"], "");
    assert_eq!(r.out(), "1,2,3!");
    let r = t::run("seq", &["-s", " ", "1", "3"], "");
    assert_eq!(r.out(), "1 2 3\n");
    let r = t::run("seq", &["-t", "", "1", "3"], "");
    assert_eq!(r.out(), "1\n2\n3");
}

#[test]
fn separator_is_taken_literally() {
    let r = t::run("seq", &["--separator=\\t", "1", "3"], "");
    assert_eq!(r.out(), "1\\t2\\t3\n");
}

#[test]
fn equal_width_pads_with_zeros() {
    let r = t::run("seq", &["-w", "1", "10"], "");
    assert_eq!(r.out(), "01\n02\n03\n04\n05\n06\n07\n08\n09\n10\n");
    let r = t::run("seq", &["--equal-width", "9", "11"], "");
    assert_eq!(r.out(), "09\n10\n11\n");
}

#[test]
fn equal_width_counts_all_three_operands() {
    // width comes from the widest operand, even when it is not printed
    let r = t::run("seq", &["-w", "3", "100", "5"], "");
    assert_eq!(r.out(), "003\n");
    let r = t::run("seq", &["-w", "9.999", "10.001"], "");
    assert_eq!(r.out(), "09.999\n");
    let r = t::run("seq", &["-w", "1", "1", "100"], "");
    let out = r.out();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 100);
    assert_eq!(lines[0], "001");
    assert_eq!(lines[99], "100");
}

#[test]
fn equal_width_pads_after_the_sign() {
    let r = t::run("seq", &["-w", "-10", "10"], "");
    assert_eq!(
        r.out(),
        "-10\n-09\n-08\n-07\n-06\n-05\n-04\n-03\n-02\n-01\n000\n001\n002\n003\n004\n005\n006\n007\n008\n009\n010\n"
    );
}

#[test]
fn equal_width_with_decimals() {
    let r = t::run("seq", &["-w", "1", "0.5", "10"], "");
    assert_eq!(
        r.out(),
        "01.0\n01.5\n02.0\n02.5\n03.0\n03.5\n04.0\n04.5\n05.0\n05.5\n06.0\n06.5\n07.0\n07.5\n08.0\n08.5\n09.0\n09.5\n10.0\n"
    );
    let r = t::run("seq", &["-w", "1", "0.5", "100"], "");
    assert!(r.out().starts_with("001.0\n001.5\n002.0\n"), "{}", r.out().lines().next().unwrap());
    assert_eq!(r.out().lines().last().unwrap(), "100.0");
    let r = t::run("seq", &["-w", "1.5", "1", "3.5"], "");
    assert_eq!(r.out(), "1.5\n2.5\n3.5\n");
    let r = t::run("seq", &["-w", "1", "-1e-3", "0.999"], "");
    assert_eq!(r.out(), "01.000\n00.999\n");
}

#[test]
fn format_fixed_and_general() {
    let r = t::run("seq", &["-f", "%.1f", "1", "3"], "");
    assert_eq!(r.out(), "1.0\n2.0\n3.0\n");
    let r = t::run("seq", &["-f", "%g", "1", "3"], "");
    assert_eq!(r.out(), "1\n2\n3\n");
    let r = t::run("seq", &["-f", "%#.0f", "1", "3"], "");
    assert_eq!(r.out(), "1.\n2.\n3.\n");
    let r = t::run("seq", &["-f", "%.1g", "1", "10"], "");
    assert_eq!(r.out(), "1\n2\n3\n4\n5\n6\n7\n8\n9\n1e+01\n");
    let r = t::run("seq", &["-f", "%.6g", "100000", "100001"], "");
    assert_eq!(r.out(), "100000\n100001\n");
}

#[test]
fn format_flags_and_widths() {
    let r = t::run("seq", &["-f", "%05.2f", "1", "3"], "");
    assert_eq!(r.out(), "01.00\n02.00\n03.00\n");
    let r = t::run("seq", &["-f", "%+08.2f", "1", "3"], "");
    assert_eq!(r.out(), "+0001.00\n+0002.00\n+0003.00\n");
    let r = t::run("seq", &["-f", "%5.2f", "1", "3"], "");
    assert_eq!(r.out(), " 1.00\n 2.00\n 3.00\n");
    let r = t::run("seq", &["-f", "%-5.2f", "1", "3"], "");
    assert_eq!(r.out(), "1.00 \n2.00 \n3.00 \n");
    let r = t::run("seq", &["-f", "%08g", "1", "2"], "");
    assert_eq!(r.out(), "00000001\n00000002\n");
}

#[test]
fn format_exponent_conversions() {
    let r = t::run("seq", &["-f", "%e", "1", "2"], "");
    assert_eq!(r.out(), "1.000000e+00\n2.000000e+00\n");
    let r = t::run("seq", &["-f", "%E", "1", "2"], "");
    assert_eq!(r.out(), "1.000000E+00\n2.000000E+00\n");
    let r = t::run("seq", &["-f", "%G", "10", "12"], "");
    assert_eq!(r.out(), "10\n11\n12\n");
}

#[test]
fn format_keeps_surrounding_text() {
    let r = t::run("seq", &["-f", "a%gb", "1", "3"], "");
    assert_eq!(r.out(), "a1b\na2b\na3b\n");
    let r = t::run("seq", &["-f", "%g%%", "1", "3"], "");
    assert_eq!(r.out(), "1%\n2%\n3%\n");
}

#[test]
fn format_length_modifiers_are_accepted() {
    let r = t::run("seq", &["-f", "%hg", "1", "3"], "");
    assert_eq!(r.out(), "1\n2\n3\n");
    let r = t::run("seq", &["-f", "%Lf", "1", "2"], "");
    assert_eq!(r.out(), "1.000000\n2.000000\n");
}

#[test]
fn format_without_directive_is_an_error() {
    for spec in ["%%", "", "abc", "%%g"] {
        let r = t::run("seq", &["-f", spec, "1", "3"], "");
        assert_eq!(r.code, 1, "spec {}", spec);
        assert!(r.err().starts_with("seq: format '"), "{}", r.err());
        assert!(r.err().contains("has no % directive"), "{}", r.err());
    }
}

#[test]
fn format_with_non_float_directive_is_an_error() {
    for spec in ["%d", "%s", "%u", "%5d", "%jd"] {
        let r = t::run("seq", &["-f", spec, "1", "3"], "");
        assert_eq!(r.code, 1, "spec {}", spec);
        assert!(
            r.err().starts_with("seq: wrong % directive type was given\n"),
            "{}",
            r.err()
        );
    }
}

#[test]
fn unknown_directive_letter_is_an_error() {
    for spec in ["%z", "%p", "%n", "%B"] {
        let r = t::run("seq", &["-f", spec, "1", "3"], "");
        assert_eq!(r.code, 1, "spec {}", spec);
        assert!(
            r.err().starts_with("seq: %: invalid conversion specification")
                || r.err().contains("invalid conversion specification"),
            "{}",
            r.err()
        );
    }
}

#[test]
fn format_with_too_many_directives_is_an_error() {
    let r = t::run("seq", &["-f", "%g%g", "1", "3"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("too many % directives"), "{}", r.err());
}

#[test]
fn format_ending_in_percent_is_an_error() {
    let r = t::run("seq", &["-f", "%", "1", "3"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("ends in %"), "{}", r.err());
    let r = t::run("seq", &["-f", "%g%", "1", "3"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("too many % directives"), "{}", r.err());
}

#[test]
fn incomplete_format_directive_is_an_error() {
    let r = t::run("seq", &["-f", "%10", "1", "3"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().starts_with("seq: %1: invalid conversion specification"), "{}", r.err());
    let r = t::run("seq", &["-f", "%.", "1", "3"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid conversion specification"), "{}", r.err());
}

#[test]
fn width_and_format_conflict() {
    let r = t::run("seq", &["-w", "-f", "%g", "1", "3"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("may not be specified when printing equal width"), "{}", r.err());
}

#[test]
fn hex_operands_are_accepted() {
    let r = t::run("seq", &["0x10"], "");
    assert_eq!(r.out(), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n");
    let r = t::run("seq", &["0x1.8", "1", "3"], "");
    assert_eq!(r.out(), "1.5\n2.5\n");
    let r = t::run("seq", &["0x1.8p2", "1", "7"], "");
    assert_eq!(r.out(), "6\n7\n");
    let r = t::run("seq", &["0x10", "0x12"], "");
    assert_eq!(r.out(), "16\n17\n18\n");
}

#[test]
fn fractional_hex_operands_print_like_percent_g() {
    let r = t::run("seq", &["1.5", "0.25", "0x2.8"], "");
    assert_eq!(r.out(), "1.5\n1.75\n2\n2.25\n2.5\n");
    let r = t::run("seq", &["0x1p-4", "0.5", "2"], "");
    assert_eq!(r.out(), "0.0625\n0.5625\n1.0625\n1.5625\n");
    let r = t::run("seq", &["1", "0.1", "0x1.1999999999999998"], "");
    assert_eq!(r.out(), "1\n");
}

#[test]
fn hex_operands_are_not_padded_by_equal_width() {
    let r = t::run("seq", &["-w", "0x10"], "");
    assert_eq!(r.out(), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n");
    let r = t::run("seq", &["-w", "1", "1", "0x10"], "");
    assert_eq!(r.out(), "1\n2\n3\n4\n5\n6\n7\n8\n9\n10\n11\n12\n13\n14\n15\n16\n");
    let r = t::run("seq", &["-w", "1", "1", "16"], "");
    assert_eq!(r.out(), "01\n02\n03\n04\n05\n06\n07\n08\n09\n10\n11\n12\n13\n14\n15\n16\n");
}

#[test]
fn big_decimal_operands_beyond_f64() {
    // magnitudes that overflow f64 are still exact in the decimal path
    let r = t::run("seq", &["1e1200", "1e1200"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "1".to_string() + &"0".repeat(1200) + "\n");
    let r = t::run("seq", &["0", "1e1200", "2e1200"], "");
    assert_eq!(r.code, 0);
    let out = r.out();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "0");
    assert_eq!(lines[1], "1".to_string() + &"0".repeat(1200));
    assert_eq!(lines[2], "2".to_string() + &"0".repeat(1200));
    let r = t::run("seq", &["-1e1200", "1e1200", "1e1200"], "");
    assert_eq!(r.code, 0);
    let out = r.out();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines.len(), 3);
    assert_eq!(lines[0], "-".to_string() + &"1".to_string() + &"0".repeat(1200));
    assert_eq!(lines[1], "0");
    assert_eq!(lines[2], "1".to_string() + &"0".repeat(1200));
}

#[test]
fn big_decimal_formats() {
    let r = t::run("seq", &["-f", "%g", "1e1200", "1e1200", "1e1200"], "");
    assert_eq!(r.out(), "1e+1200\n");
    let r = t::run("seq", &["-f", "%.2e", "1e1200", "1e1200", "1e1200"], "");
    assert_eq!(r.out(), "1.00e+1200\n");
    let r = t::run("seq", &["-w", "1e1200", "1e1200", "1e1200"], "");
    assert_eq!(r.out(), "1".to_string() + &"0".repeat(1200) + "\n");
    let r = t::run("seq", &["1e-1200", "1e-1200"], "");
    let out = r.out();
    let line = out.lines().next().unwrap();
    assert_eq!(line.len(), 1202);
    assert!(line.starts_with("0.0000"));
    assert!(line.ends_with('1'));
}

#[test]
fn missing_operand_is_an_error() {
    let r = t::run("seq", &[], "");
    assert_eq!(r.code, 1);
    assert!(r.err().starts_with("seq: missing operand"), "{}", r.err());
    let r = t::run("seq", &["-s", ","], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("missing operand"), "{}", r.err());
}

#[test]
fn zero_increment_is_an_error() {
    let r = t::run("seq", &["1", "0", "5"], "");
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with("seq: invalid Zero increment value: '0'"),
        "{}",
        r.err()
    );
    let r = t::run("seq", &["1", "-0", "3"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid Zero increment value: '-0'"), "{}", r.err());
}

#[test]
fn invalid_number_is_an_error() {
    let r = t::run("seq", &["a"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().starts_with("seq: invalid floating point argument: 'a'"), "{}", r.err());
    let r = t::run("seq", &["1", "b", "3"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid floating point argument: 'b'"), "{}", r.err());
    let r = t::run("seq", &["nan"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().starts_with("seq: invalid 'not-a-number' argument: 'nan'"), "{}", r.err());
}

#[test]
fn infinite_operands_stay_in_the_float_path() {
    let r = t::run("seq", &["inf", "1", "2"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "");
    let r = t::run("seq", &["1", "inf", "3"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "1\n");
}

#[test]
fn hyphen_tokens_before_positionals_become_operands() {
    let r = t::run("seq", &["-x", "1", "3"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid floating point argument: '-x'"), "{}", r.err());
    let r = t::run("seq", &["--w", "1", "3"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid floating point argument: '--w'"), "{}", r.err());
}

#[test]
fn too_many_positionals_is_a_clap_error() {
    let r = t::run("seq", &["1", "2", "3", "4"], "");
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with("error: unexpected value '4' for '[numbers]...' found"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("Usage: seq [OPTION]... LAST"), "{}", r.err());
    // after a positional, options land as positionals too
    let r = t::run("seq", &["1", "2", "3", "--version"], "");
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with("error: unexpected value '--version' for '[numbers]...' found"),
        "{}",
        r.err()
    );
    let r = t::run("seq", &["1", "3", "-s,"], "");
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with("error: unexpected value ',' for '[numbers]...' found"),
        "{}",
        r.err()
    );
}

#[test]
fn duplicate_option_is_a_clap_error() {
    let r = t::run("seq", &["-w", "-w", "1", "3"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("'--equal-width' cannot be used multiple times"), "{}", r.err());
    let r = t::run("seq", &["-s", "x", "-s", "y", "1", "3"], "");
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("'--separator <separator>' cannot be used multiple times"),
        "{}",
        r.err()
    );
}

#[test]
fn missing_option_value_is_a_clap_error() {
    let r = t::run("seq", &["--format"], "");
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with("error: a value is required for '--format <format>' but none was supplied"),
        "{}",
        r.err()
    );
    let r = t::run("seq", &["-s", "-w", "3"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("a value is required for '--separator <separator>'"), "{}", r.err());
}

#[test]
fn help_and_version_short_circuit() {
    let r = t::run("seq", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Display numbers from FIRST to LAST"), "{}", r.out());
    assert!(r.out().contains("-w, --equal-width"), "{}", r.out());
    let v = t::run("seq", &["-V"], "");
    assert_eq!(v.code, 0);
    assert!(v.out().starts_with("seq "), "{}", v.out());
    // the first action wins, before the operands are looked at
    let r = t::run("seq", &["-h", "1", "2", "3", "4"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Usage: seq [OPTION]... LAST"), "{}", r.out());
    let r = t::run("seq", &["-Vh"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().starts_with("seq "), "{}", r.out());
    let r = t::run("seq", &["-hV"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Usage: seq"), "{}", r.out());
}

#[test]
fn abbreviated_long_options_match() {
    let r = t::run("seq", &["--he"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Usage: seq"), "{}", r.out());
    let r = t::run("seq", &["--ver"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().starts_with("seq "), "{}", r.out());
    let r = t::run("seq", &["--sep=x", "1", "3"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "1x2x3\n");
}

#[test]
fn short_cluster_with_attached_values() {
    let r = t::run("seq", &["-s,", "1", "3"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "1,2,3\n");
    let r = t::run("seq", &["-sxy", "1", "3"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "1xy2xy3\n");
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("seq", &["1", "3"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "1\n2\n3\n");
    let r = t::run_bin("seq", &["-w", "9", "11"], "");
    assert_eq!(r.out(), "09\n10\n11\n");
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

#[test]
fn fuzz_equal_width_is_uniform() {
    let mut rng = t::Rng::new(0x5EED);
    for _ in 0..60 {
        let first = rng.below(50) as i64;
        let incr = 1 + rng.below(7) as i64;
        let last = first + incr * (1 + rng.below(30) as i64);
        let strs = ["-w".to_string(), first.to_string(), incr.to_string(), last.to_string()];
        let a: Vec<&str> = strs.iter().map(|x| x.as_str()).collect();
        let r = t::run("seq", &a, "");
        assert_eq!(r.code, 0);
        let out = r.out();
        let lines: Vec<&str> = out.lines().collect();
        let width = lines.iter().map(|l| l.len()).max().unwrap();
        for l in &lines {
            assert_eq!(l.len(), width, "line {} width {}", l, width);
        }
    }
}

#[test]
fn fuzz_format_percent_g_roundtrip() {
    let mut rng = t::Rng::new(0xF00D);
    for _ in 0..60 {
        let first = 1 + rng.below(40) as i64;
        let last = first + 1 + rng.below(40) as i64;
        let strs = ["-f".to_string(), "%g".to_string(), first.to_string(), last.to_string()];
        let a: Vec<&str> = strs.iter().map(|x| x.as_str()).collect();
        let r = t::run("seq", &a, "");
        assert_eq!(r.code, 0);
        let out = r.out();
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), (last - first + 1) as usize);
        for (i, l) in lines.iter().enumerate() {
            assert_eq!(l.parse::<i64>().unwrap(), first + i as i64);
        }
    }
}
