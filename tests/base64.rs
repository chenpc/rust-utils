use coreutils::testutil as t;

fn b64(args: &[&str], input: &str) -> t::Out {
    t::run("base64", args, input)
}

#[test]
fn encodes_known_vector() {
    assert_eq!(b64(&[], "hello world\n").out(), "aGVsbG8gd29ybGQK\n");
}

#[test]
fn decodes_known_vector() {
    assert_eq!(b64(&["-d"], "aGVsbG8gd29ybGQK\n").out(), "hello world\n");
}

#[test]
fn wraps_at_76_by_default() {
    let long = "a".repeat(120) + "\n";
    let r2 = b64(&[], &long);
    let out2 = r2.out();
    let l2: Vec<&str> = out2.lines().collect();
    assert_eq!(l2[0].len(), 76);
    assert_eq!(l2[1].len(), 76);
    assert_eq!(l2[2].len(), 12);
}

#[test]
fn wrap_zero_disables_wrapping() {
    let long = "a".repeat(120) + "\n";
    let r = b64(&["-w", "0"], &long);
    let out = r.out();
    assert_eq!(out.lines().count(), 1);
}

#[test]
fn attached_wrap_value() {
    let long = "a".repeat(120) + "\n";
    let r = b64(&["-w60"], &long);
    let out = r.out();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0].len(), 60);
}

#[test]
fn newlines_in_input_are_ignored_when_decoding() {
    assert_eq!(b64(&["-d"], "aGVsbG8g\nd29ybGQ=\n").out(), "hello world");
}

#[test]
fn invalid_char_without_ignore_exit_1() {
    let r = b64(&["-d"], "aGVsbG8gd29ybGQK!x\n");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid input"));
}

#[test]
fn ignore_garbage_skips_invalid_chars() {
    assert_eq!(b64(&["-d", "-i"], "aGVsbG8gd29ybGQK!x\n").out(), "hello world\n");
}

#[test]
fn missing_file_exit_1() {
    let r = b64(&["/definitely/not/here"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("base64:"));
}

#[test]
fn missing_operand_and_unknown_option_exit_2() {
    assert_eq!(b64(&["-w"], "x\n").code, 2);
    assert_eq!(b64(&["-Z"], "x\n").code, 2);
    assert_eq!(b64(&["-w", "abc"], "x\n").code, 2);
}

#[test]
fn help_and_version() {
    let r = b64(&["-h"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("base64"));
    let v = b64(&["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.out().contains("base64"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("base64", &[], "hi\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "aGkK\n");
}

#[test]
fn fuzz_roundtrip() {
    let mut rng = t::Rng::new(0xB640);
    for _ in 0..100 {
        let len = rng.below(40);
        let input = rng.bytes(len as usize);
        let enc = t::run_bytes("base64", &["-w", "0"], &input);
        assert_eq!(enc.code, 0);
        let dec = t::run_bytes("base64", &["-d"], &enc.stdout);
        assert_eq!(dec.code, 0);
        assert_eq!(dec.stdout, input);
    }
}

#[test]
fn fuzz_output_only_uses_alphabet_and_newline() {
    let mut rng = t::Rng::new(0xB641);
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/=";
    for _ in 0..60 {
        let len = rng.below(30) as usize;
        let input = rng.bytes(len);
        let r = t::run_bytes("base64", &[], &input);
        assert_eq!(r.code, 0);
        for c in r.out().chars() {
            assert!(alphabet.contains(c) || c == '\n', "bad char {:?}", c);
        }
    }
}