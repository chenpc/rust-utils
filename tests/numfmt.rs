use coreutils::testutil as t;

fn r(args: &[&str], input: &str) -> t::Out {
    t::run("numfmt", args, input)
}

// man page examples
#[test]
fn man_example_from_si() {
    assert_eq!(r(&["--from", "si", "1.5K"], "").out(), "1500\n");
}

#[test]
fn man_example_to_si() {
    assert_eq!(r(&["--to", "si", "1500"], "").out(), "1.5K\n");
}

#[test]
fn man_example_from_iec() {
    assert_eq!(r(&["--from", "iec", "1.5K"], "").out(), "1536\n");
}

#[test]
fn man_example_from_auto_iec_i() {
    assert_eq!(r(&["--from", "auto", "1.5Ki"], "").out(), "1536\n");
}

#[test]
fn auto_single_letter_is_si() {
    assert_eq!(r(&["--from", "auto", "1.5K"], "").out(), "1500\n");
}

#[test]
fn to_iec_rounds_mantissa() {
    assert_eq!(r(&["--to", "iec", "1536"], "").out(), "1.5K\n");
}

#[test]
fn no_unit_is_passthrough() {
    assert_eq!(r(&["42"], "").out(), "42\n");
    assert_eq!(r(&["1.25"], "").out(), "1.25\n");
}

#[test]
fn suffix_triggers_error_when_from_none() {
    let got = r(&["1K"], "");
    assert_eq!(got.code, 2);
    assert!(got.err().contains("invalid number"));
}

#[test]
fn reads_numbers_from_stdin() {
    let got = r(&["--to", "si"], "1500\n2000\n");
    assert_eq!(got.out(), "1.5K\n2K\n");
}

#[test]
fn field_selection() {
    let got = r(&["--field", "2", "--to", "si"], "a 1500 b\n");
    assert_eq!(got.out(), "a 1.5K b\n");
}

#[test]
fn field_range() {
    let got = r(&["--field", "1-2", "--to", "si"], "1000 2000 3000\n");
    assert_eq!(got.out(), "1K 2K 3000\n");
}

#[test]
fn field_from_n_to_end() {
    let got = r(&["--field", "2-", "--to", "si"], "x 1000 2000\n");
    assert_eq!(got.out(), "x 1K 2K\n");
}

#[test]
fn comma_separated_fields() {
    let got = r(&["--field", "1,3", "--to", "si"], "1000 x 2000\n");
    assert_eq!(got.out(), "1K x 2K\n");
}

#[test]
fn header_is_printed_without_converting() {
    let got = r(&["--header=1", "--to", "si"], "hdr 1000\n1000\n");
    assert_eq!(got.out(), "hdr 1000\n1K\n");
}

#[test]
fn header_with_count() {
    let got = r(&["--header=2", "--to", "si"], "a\nb\n1000\n");
    assert_eq!(got.out(), "a\nb\n1K\n");
}

#[test]
fn padding_right_aligns() {
    let got = r(&["--padding", "5", "7"], "");
    assert_eq!(got.out(), "    7\n");
}

#[test]
fn padding_left_aligns() {
    let got = r(&["--padding", "-5", "7"], "");
    assert_eq!(got.out(), "7    \n");
}

#[test]
fn padding_ignored_when_wider() {
    let got = r(&["--padding", "2", "12345"], "");
    assert_eq!(got.out(), "12345\n");
}

#[test]
fn format_width_and_precision() {
    assert_eq!(r(&["--format", "%10f", "1.5"], "").out(), "       1.5\n");
    assert_eq!(r(&["--format", "%-10f", "1.5"], "").out(), "1.5       \n");
    assert_eq!(r(&["--format", "%.2f", "1.5"], "").out(), "1.50\n");
    assert_eq!(r(&["--format", "%010f", "1.5"], "").out(), "00000001.5\n");
}

#[test]
fn format_quote_enables_grouping() {
    assert_eq!(r(&["--format", "%'f", "1000000"], "").out(), "1,000,000\n");
}

#[test]
fn grouping_option() {
    assert_eq!(r(&["--grouping", "1000000"], "").out(), "1,000,000\n");
    assert_eq!(r(&["1000000"], "").out(), "1000000\n");
}

#[test]
fn suffix_option() {
    let got = r(&["--suffix", "B", "--to", "si", "1500"], "");
    assert_eq!(got.out(), "1.5KB\n");
    // --suffix is printed after each formatted number, so the input suffix is accepted and re-appended
    let got2 = r(&["--suffix", "B", "--from", "si", "2KB"], "");
    assert_eq!(got2.out(), "2000B\n");
}

#[test]
fn unit_separator() {
    let got = r(&["--unit-separator", " ", "--to", "si", "1500"], "");
    assert_eq!(got.out(), "1.5 K\n");
}

#[test]
fn rounding_methods() {
    // rounding applies at the output precision, so force precision 0 to see its effect
    assert_eq!(r(&["--from", "si", "--to", "si", "--round", "from-zero", "--format", "%.0f", "1.5K"], "").out(), "2K\n");
    assert_eq!(r(&["--from", "si", "--to", "si", "--round", "towards-zero", "--format", "%.0f", "1.5K"], "").out(), "1K\n");
    assert_eq!(r(&["--from", "si", "--to", "si", "--round", "up", "--format", "%.0f", "1.2K"], "").out(), "2K\n");
    assert_eq!(r(&["--from", "si", "--to", "si", "--round", "down", "--format", "%.0f", "1.8K"], "").out(), "1K\n");
    assert_eq!(r(&["--from", "si", "--to", "si", "--round", "nearest", "--format", "%.0f", "1.5K"], "").out(), "2K\n");
    // at precision 1 rounding does not change the mantissa
    assert_eq!(r(&["--from", "si", "--to", "si", "1.5K"], "").out(), "1.5K\n");
}

#[test]
fn negative_numbers() {
    assert_eq!(r(&["--to", "si", "-1500"], "").out(), "-1.5K\n");
    assert_eq!(r(&["--from", "si", "-1.5K"], "").out(), "-1500\n");
}

#[test]
fn delimiter_option() {
    let got = r(&["-d", ",", "--field", "2", "--to", "si", "a,1500"], "");
    assert_eq!(got.out(), "a,1.5K\n");
}

#[test]
fn zero_terminated_input() {
    let got = t::run_bytes("numfmt", &["-z", "--to", "si"], b"1500\02000\0");
    assert_eq!(got.out(), "1.5K\n2K\n");
}

#[test]
fn invalid_unit_is_an_error() {
    let got = r(&["--from", "xyz", "1"], "");
    assert_eq!(got.code, 2);
    assert!(got.err().contains("invalid unit"));
}

#[test]
fn invalid_round_is_an_error() {
    let got = r(&["--round", "xyz", "1"], "");
    assert_eq!(got.code, 2);
    assert!(got.err().contains("rounding"));
}

#[test]
fn missing_operand_is_an_error() {
    assert_eq!(r(&["--to"], "").code, 2);
}

#[test]
fn help_and_version() {
    let h = r(&["--help"], "");
    assert_eq!(h.code, 0);
    assert!(h.out().contains("numfmt"));
    let v = r(&["-V"], "");
    assert_eq!(v.code, 0);
    assert!(v.out().contains("numfmt"));
}

#[test]
fn e2e_binary_dispatch() {
    let o = t::run_bin("numfmt", &["--to", "si", "1500"], "");
    assert_eq!(o.out(), "1.5K\n");
}

#[test]
fn fuzz_si_roundtrip() {
    let mut rng = t::Rng::new(0xBEEF);
    for _ in 0..150 {
        let n = rng.below(1000000) as i64;
        // n -> --to si -> --from si 必須回到 n（precision 1 時可能差一個 mantissa 誤差）
        let scaled = r(&["--to", "si", &n.to_string()], "").out().trim().to_string();
        let back = r(&["--from", "si", &scaled], "").out().trim().to_string();
        let back_val: f64 = back.parse().unwrap_or(-1.0);

        assert!((back_val - n as f64).abs() <= 1000.0, "roundtrip {n} -> {scaled} -> {back}");
    }
}

#[test]
fn fuzz_iec_vs_si_differ_by_24_percent() {
    let mut rng = t::Rng::new(0xF00D);
    for _ in 0..120 {
        let exp = 1 + rng.below(4) as usize;
        let mant = 1 + rng.below(9) as i64;
        let label = ["K", "M", "G", "T", "P", "E"][exp - 1];
        let text = format!("{mant}{label}");
        let si: i64 = r(&["--from", "si", &text], "").out().trim().parse().unwrap();
        let iec: i64 = r(&["--from", "iec", &text], "").out().trim().parse().unwrap();
        let mut base = 1i64;
        let mut bin = 1i64;
        for _ in 0..exp {
            base *= 1000;
            bin *= 1024;
        }
        assert_eq!(si, mant * base, "si {text}");
        assert_eq!(iec, mant * bin, "iec {text}");
    }
}

#[test]
fn fuzz_padding_never_shrinks() {
    let mut rng = t::Rng::new(0x5A5A);
    for _ in 0..120 {
        let n = rng.below(100000) as i64;
        let pad = 1 + rng.below(20) as i64;
        let got = r(&["--padding", &pad.to_string(), &n.to_string()], "").out();
        let line = got.trim_end_matches('\n');
        assert!(line.len() as i64 >= pad, "pad {pad} got {line}");
        assert!(line.contains(&n.to_string()), "value preserved");
    }
}
