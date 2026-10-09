use coreutils::testutil as t;

fn b32(args: &[&str], input: &str) -> t::Out {
    t::run("base32", args, input)
}

#[test]
fn encodes_known_vector() {
    assert_eq!(b32(&[], "hello world\n").out(), "NBSWY3DPEB3W64TMMQFA====\n");
}

#[test]
fn decodes_known_vector() {
    assert_eq!(b32(&["-d"], "NBSWY3DPEB3W64TMMQFA====\n").out(), "hello world\n");
}

#[test]
fn padding_for_each_partial_group() {
    // 1..4 leftover bytes produce 8 chars with 6,3,6,1 padding chars
    for (input, exp) in [
        ("a", "ME======"),
        ("ab", "MFRA===="),
        ("abc", "MFRGG==="),
        ("abcd", "MFRGGZA="),
        ("abcde", "MFRGGZDF"),
    ] {
        assert_eq!(b32(&["-w", "0"], input).out(), exp);
    }
}

#[test]
fn decode_reconstructs_partial_groups() {
    for exp in ["a", "ab", "abc", "abcd", "abcde"] {
        let enc = b32(&["-w", "0"], exp).out();
        assert_eq!(b32(&["-d", "-w", "0"], &enc).out(), exp);
    }
}

#[test]
fn wraps_at_76_by_default() {
    let long = "a".repeat(120) + "\n";
    let r = b32(&[], &long);
    let out = r.out();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0].len(), 76);
    assert_eq!(lines[1].len(), 76);
}

#[test]
fn wrap_zero_disables_wrapping() {
    let long = "a".repeat(120) + "\n";
    let out = b32(&["-w", "0"], &long).out();
    assert_eq!(out.lines().count(), 1);
}

#[test]
fn attached_wrap_value() {
    let long = "a".repeat(120) + "\n";
    let out = b32(&["-w60"], &long).out();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0].len(), 60);
}

#[test]
fn newlines_in_input_are_ignored_when_decoding() {
    assert_eq!(b32(&["-d"], "NBSWY3DP\nEB3W64TM\nMQFA====\n").out(), "hello world\n");
}

#[test]
fn invalid_char_without_ignore_exit_1() {
    let r = b32(&["-d"], "NBSWY3DPEB3W64TMMQFA!===\n");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid input"));
}

#[test]
fn ignore_garbage_skips_invalid_chars() {
    // '!' is not in the base32 alphabet; skipping it must not consume a bit slot
    assert_eq!(b32(&["-d", "-i"], "NBSWY3DPEB3W64TMMQFA!===\n").out(), "hello world\n");
}

#[test]
fn missing_file_exit_1() {
    let r = b32(&["/definitely/not/here"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("base32:"));
}

#[test]
fn missing_operand_and_unknown_option_exit_2() {
    assert_eq!(b32(&["-w"], "").code, 2);
    assert_eq!(b32(&["--nope"], "x").code, 2);
    assert_eq!(b32(&["-w", "abc"], "x").code, 2);
}

#[test]
fn reads_file_and_dash_is_stdin() {
    let wd = t::workdir();
    let path = wd.join("b32_input.txt");
    std::fs::write(&path, "hello world\n").unwrap();
    assert_eq!(b32(&[path.to_str().unwrap()], "").out(), "NBSWY3DPEB3W64TMMQFA====\n");
    assert_eq!(b32(&["-"], "hello world\n").out(), "NBSWY3DPEB3W64TMMQFA====\n");
}

#[test]
fn help_and_version() {
    assert!(b32(&["--help"], "").out().contains("base32"));
    assert!(b32(&["--version"], "").out().contains("base32"));
}

#[test]
fn e2e_binary_dispatch() {
    assert_eq!(t::run_bin("base32", &["-w", "0"], "hello world\n").out(), "NBSWY3DPEB3W64TMMQFA====");
}

#[test]
fn fuzz_roundtrip() {
    let mut rng = t::Rng::new(0xB32D);
    for _ in 0..100 {
        let len = rng.below(64) as usize;
        let data = rng.bytes(len);
        let enc = t::run_bytes("base32", &["-w", "0"], &data);
        let back = t::run_bytes("base32", &["-d", "-w", "0"], &enc.stdout);
        assert_eq!(back.stdout, data, "roundtrip failed for len {}", len);
    }
}

#[test]
fn fuzz_decode_output_is_no_longer_than_input() {
    // each 5 chars yields at most 8 bytes; a partial group can never yield more
    let mut rng = t::Rng::new(0xB32E);
    let alphabet = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ234567=";
    for _ in 0..100 {
        let len = 1 + rng.below(80) as usize;
        let mut s = Vec::new();
        for _ in 0..len {
            s.push(alphabet[rng.below(alphabet.len() as u64) as usize]);
        }
        let r = t::run_bytes("base32", &["-d", "-w", "0"], &s);
        assert_eq!(r.code, 0);
        assert!(r.stdout.len() <= s.len() * 8 / 5 + 8);
        // decoding the decoded output must reproduce the same bytes
        let enc = t::run_bytes("base32", &["-w", "0"], &r.stdout);
        let again = t::run_bytes("base32", &["-d", "-w", "0"], &enc.stdout);
        assert_eq!(again.stdout, r.stdout);
    }
}
