use coreutils::testutil as t;

fn hex_of(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect::<String>()
}

#[test]
fn default_is_octal_two_byte_units() {
    let r = t::run_bytes("od", &[], b"abc");
    // 2-byte little-endian octal: 'ab' = 0x6261 = 061141, then lone 'c' = 000143
    assert_eq!(r.out(), "0000000 061141 000143\n0000003\n");
}

#[test]
fn hex_one_byte_format() {
    let r = t::run_bytes("od", &["-tx1"], b"abc");
    assert_eq!(r.out(), "0000000 61 62 63\n0000003\n");
}

#[test]
fn attached_value_form() {
    let r = t::run_bytes("od", &["-An", "-tx1"], b"abc");
    assert_eq!(r.out(), " 61 62 63\n");
}

#[test]
fn character_format_with_escapes() {
    let r = t::run_bytes("od", &["-c"], b"a\nb\\\0");
    assert_eq!(r.out(), "0000000   a  \\n   b  \\\\  \\0\n0000005\n");
}

#[test]
fn named_character_format() {
    let r = t::run_bytes("od", &["-ta"], b"\0a");
    assert_eq!(r.out(), "0000000 nul 141\n0000002\n");
}

#[test]
fn width_splits_lines() {
    let r = t::run_bytes("od", &["-tx1", "-w", "2"], b"abcd");
    assert_eq!(r.out(), "0000000 61 62\n0000002 63 64\n0000004\n");
}

#[test]
fn width_without_value_is_32() {
    let r = t::run_bytes("od", &["-tx1", "-w"], b"ab");
    assert_eq!(r.out(), "0000000 61 62\n0000002\n");
}

#[test]
fn read_bytes_limits_dump() {
    let r = t::run_bytes("od", &["-tx1", "-N", "2"], b"abcd");
    assert_eq!(r.out(), "0000000 61 62\n0000002\n");
}

#[test]
fn skip_bytes() {
    let r = t::run_bytes("od", &["-tx1", "-j", "2"], b"abcd");
    assert_eq!(r.out(), "0000002 63 64\n0000004\n");
}

#[test]
fn address_radix_decimal_and_none() {
    let r = t::run_bytes("od", &["-tx1", "-A", "d"], b"ab");
    assert_eq!(r.out(), "00000000 61 62\n00000002\n");
    let r2 = t::run_bytes("od", &["-tx1", "-An"], b"ab");
    assert_eq!(r2.out(), " 61 62\n");
}

#[test]
fn duplicate_lines_are_marked_with_star() {
    let input = b"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"; // 32 bytes, two identical 16-byte lines
    let r = t::run_bytes("od", &["-tx1", "-w", "16"], input);
    let first = "0000000 61 61 61 61 61 61 61 61 61 61 61 61 61 61 61 61\n";
    assert_eq!(r.out(), format!("{}*\n0000040\n", first));
    let v = t::run_bytes("od", &["-tx1", "-w", "16", "-v"], input);
    let second = first.replace("0000000", "0000020");
    assert_eq!(v.out(), format!("{}{}0000040\n", first, second));
}

#[test]
fn multiple_formats_print_one_line_each() {
    let r = t::run_bytes("od", &["-tx1", "-c"], b"ab");
    assert_eq!(
        r.out(),
        "0000000 61 62\n   a   b\n0000002\n"
    );
}

#[test]
fn z_suffix_appends_ascii_dump() {
    let r = t::run_bytes("od", &["-tx1z"], b"ab\x01");
    assert_eq!(r.out(), "0000000 61 62 01 ab.\n0000003\n");
}

#[test]
fn endian_big() {
    let r = t::run_bytes("od", &["-tx2", "--endian", "big"], b"abcd");
    assert_eq!(r.out(), "0000000 6162 6364\n0000004\n");
    let l = t::run_bytes("od", &["-tx2", "--endian", "little"], b"abcd");
    assert_eq!(l.out(), "0000000 6261 6463\n0000004\n");
}

#[test]
fn strings_option_prints_graphic_runs() {
    let r = t::run_bytes("od", &["-S"], b"ab\x01cd\x02defg");
    assert_eq!(r.out(), "defg\n");
}

#[test]
fn traditional_omits_size_line() {
    let r = t::run_bytes("od", &["--traditional", "-tx1"], b"ab");
    assert_eq!(r.out(), "0000000 61 62\n");
}

#[test]
fn double_dash_stops_option_parsing() {
    let r = t::run_bytes("od", &["--", "-tx1"], b"");
    assert_eq!(r.err(), "od: -tx1: No such file or directory (os error 2)\n");
    assert_eq!(r.code, 1);
}

#[test]
fn missing_file_is_error() {
    let r = t::run_bytes("od", &["/definitely-missing"], b"");
    assert_eq!(r.code, 1);
    assert!(r.err().starts_with("od: /definitely-missing:"));
}

#[test]
fn dash_reads_stdin() {
    let r = t::run_bytes("od", &["-tx1", "-"], b"ab");
    assert_eq!(r.out(), "0000000 61 62\n0000002\n");
}

#[test]
fn help_and_version() {
    let r = t::run_bytes("od", &["--help"], b"");
    assert!(r.out().contains("od"));
    assert_eq!(r.code, 0);
    let v = t::run_bytes("od", &["--version"], b"");
    assert!(v.out().contains("od"));
    assert_eq!(v.code, 0);
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("od", &["-An", "-tx1"], "hi");
    assert_eq!(r.out(), " 68 69\n");
}

#[test]
fn fuzz_hex_format_reconstructs_input() {
    for seed in 0..120u64 {
        let mut rng = t::Rng::new(seed);
        let len = (rng.below(40) + 1) as usize;
        let input = rng.bytes(len);
        let r = t::run_bytes("od", &["-An", "-tx1", "-v", "-w", "64"], &input);
        let got: String = r
            .out()
            .split_whitespace()
            .filter(|w| w.len() == 2)
            .collect::<String>();
        assert_eq!(got, hex_of(&input), "seed {}", seed);
    }
}

#[test]
fn fuzz_char_format_reconstructs_input() {
    for seed in 0..120u64 {
        let mut rng = t::Rng::new(seed);
        let len = (rng.below(30) + 1) as usize;
        let input = rng.bytes(len);
        let input: Vec<u8> = input.iter().filter(|b| **b != b' ' && **b != b'\n' && **b != b'\t' && **b != b'\r').cloned().collect();
        let r = t::run_bytes("od", &["-An", "-c", "-v", "-w", "64"], &input);
        let toks: Vec<String> = r.out().split_whitespace().map(|s| s.to_string()).collect();
        assert_eq!(toks.len(), input.len(), "seed {}", seed);
        for (i, &b) in input.iter().enumerate() {
            let want: String = match b {
                b'\n' => "\\n".to_string(),
                b'\t' => "\\t".to_string(),
                b'\r' => "\\r".to_string(),
                b'\\' => "\\\\".to_string(),
                0 => "\\0".to_string(),
                7 => "\\a".to_string(),
                8 => "\\b".to_string(),
                12 => "\\f".to_string(),
                11 => "\\v".to_string(),
                _ if b.is_ascii_graphic() || b == b' ' => (b as char).to_string(),
                _ => format!("\\{:03o}", b),
            };
            assert_eq!(toks[i], want, "seed {} byte {}", seed, b);
        }
    }
}

#[test]
fn fuzz_write_error_returns_1() {
    let mut fw = t::FailWriter::new(3);
    let mut err = Vec::new();
    let code = t::run_with("od", &["-An", "-tx1"], b"abcdefgh", &mut fw, &mut err);
    assert_eq!(code, 1);
    assert_eq!(fw.written, 3);
}

#[test]
fn size_suffixes_in_format() {
    let r = t::run_bytes("od", &["-txC"], b"ab");
    assert_eq!(r.out(), "0000000 61 62\n0000002\n");
    let r = t::run_bytes("od", &["-txS"], b"abcd");
    assert_eq!(r.out(), "0000000 6261 6463\n0000004\n");
    let r = t::run_bytes("od", &["-tdI"], b"abcdabcd");
    assert_eq!(r.out(), "0000000 1684234849 1684234849\n0000010\n");
    let r = t::run_bytes("od", &["-toL"], b"abcdabcd");
    assert_eq!(r.out(), "0000000 000621433046054430661141\n0000010\n");
    let r = t::run_bytes("od", &["-tfF", "--endian", "big"], b"\x00\x00\x80\x40");
    assert_eq!(r.out(), "0000000      0\n0000004\n");
}

#[test]
fn unsigned_decimal_format() {
    let r = t::run_bytes("od", &["-tu2"], b"\xff\xff");
    assert_eq!(r.out(), "0000000 65535\n0000002\n");
}

#[test]
fn float_format() {
    // 0x40800000 (big endian) = 2.0f32
    let r = t::run_bytes("od", &["-tf", "--endian", "big"], b"\x40\x80\x00\x00");
    assert_eq!(r.out(), "0000000      4\n0000004\n");
}

#[test]
fn address_radix_hex() {
    let r = t::run_bytes("od", &["-tx1", "-A", "x"], b"ab");
    assert_eq!(r.out(), "00000000 61 62\n00000002\n");
}

#[test]
fn byte_suffix_multipliers() {
    // 1b = 512 bytes; input is shorter so the whole input is dumped
    let r = t::run_bytes("od", &["-tx1", "-N", "1b"], b"ab");
    assert_eq!(r.out(), "0000000 61 62\n0000002\n");
    let r = t::run_bytes("od", &["-tx1", "-N", "0x2"], b"abcd");
    assert_eq!(r.out(), "0000000 61 62\n0000002\n");
    let r = t::run_bytes("od", &["-tx1", "-j", "02"], b"abcd");
    assert_eq!(r.out(), "0000002 63 64\n0000004\n");
}

#[test]
fn missing_operands_exit_2() {
    for opt in ["-A", "-j", "-N", "-t"] {
        let r = t::run_bytes("od", &[opt], b"");
        assert_eq!(r.code, 2, "option {}", opt);
        assert!(r.err().contains("missing operand"));
    }
}

#[test]
fn fuzz_unsigned_decimal_matches_bytes() {
    for seed in 0..120u64 {
        let mut rng = t::Rng::new(seed);
        let len = (rng.below(24) + 1) as usize;
        let input = rng.bytes(len);
        let r = t::run_bytes("od", &["-An", "-tu2", "-v", "-w", "64"], &input);
        let toks: Vec<String> = r.out().split_whitespace().map(|s| s.to_string()).collect();
        let mut pos = 0;
        let mut idx = 0;
        while pos < input.len() {
            let sz = 2.min(input.len() - pos);
            let mut v: u64 = 0;
            for &b in input[pos..pos + sz].iter().rev() {
                v = v * 256 + b as u64;
            }
            assert_eq!(toks[idx], v.to_string(), "seed {}", seed);
            idx += 1;
            pos += sz;
        }
    }
}
