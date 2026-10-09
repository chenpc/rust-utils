use coreutils::testutil as t;

fn enc(codec: &str, input: &str) -> t::Out {
    t::run("basenc", &[codec], input)
}

fn dec(codec: &str, input: &str) -> t::Out {
    t::run("basenc", &["-d", codec], input)
}

const DATA: &str = "hello world\n";

#[test]
fn known_vectors_for_every_encoding() {
    // -w 0 so the expected strings are not wrapped at the default 76 columns
    for (codec, exp) in [
        ("--base64", "aGVsbG8gd29ybGQK"),
        ("--base64url", "aGVsbG8gd29ybGQK"),
        ("--base32", "NBSWY3DPEB3W64TMMQFA===="),
        ("--base32hex", "D1IMOR3F41RMUSJCCG50===="),
        ("--base16", "68656C6C6F20776F726C640A"),
        (
            "--base2msbf",
            "011010000110010101101100011011000110111100100000011101110110111101110010011011000110010000001010",
        ),
        (
            "--base2lsbf",
            "000101101010011000110110001101101111011000000100111011101111011001001110001101100010011001010000",
        ),
        ("--z85", "xK#0@zY<mxA+]m["),
        ("--base58", "2yGEbwRFyhPZZckJm"),
    ] {
        assert_eq!(t::run("basenc", &[codec, "-w", "0"], DATA).out(), exp, "{}", codec);
    }
}

#[test]
fn default_encoding_is_base64() {
    assert_eq!(t::run("basenc", &[], DATA).out(), "aGVsbG8gd29ybGQK\n");
}

#[test]
fn every_encoding_roundtrips() {
    for codec in [
        "--base64",
        "--base64url",
        "--base32",
        "--base32hex",
        "--base16",
        "--base2lsbf",
        "--base2msbf",
        "--z85",
        "--base58",
    ] {
        let e = enc(codec, DATA).out();
        assert_eq!(dec(codec, &e).out(), DATA, "roundtrip failed for {}", codec);
    }
}

#[test]
fn base64url_uses_dash_and_underscore() {
    // bytes 0xFB 0xFF encode as +/ in base64 and -_ in base64url
    let raw: &[u8] = &[0xFB, 0xFF];
    assert_eq!(t::run_bytes("basenc", &[ "--base64", "-w", "0"], raw).stdout, b"+/8=");
    assert_eq!(t::run_bytes("basenc", &[ "--base64url", "-w", "0"], raw).stdout, b"-_8=");
    assert_eq!(t::run_bytes("basenc", &["-d",  "--base64url", "-w", "0"], b"-_8=").stdout, raw.to_vec());
}

#[test]
fn base32hex_alphabet() {
    // base32hex uses 0-9 then A-V instead of A-Z
    assert_eq!(t::run_bytes("basenc", &[ "--base32hex", "-w", "0"], b"a").stdout, b"C4======");
    assert_eq!(t::run_bytes("basenc", &["-d",  "--base32hex", "-w", "0"], b"C4======").stdout, b"a".to_vec());
}

#[test]
fn z85_requires_multiple_of_4_when_encoding() {
    let r = t::run("basenc", &[ "--z85", "-w", "0"], "abc");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("multiple of 4"));
}

#[test]
fn z85_requires_multiple_of_5_when_decoding() {
    let r = t::run("basenc", &["-d",  "--z85", "-w", "0"], "xK#0@");
    assert_eq!(r.code, 0);
    let r2 = t::run("basenc", &["-d",  "--z85", "-w", "0"], "xK#0@z");
    assert_eq!(r2.code, 1);
    assert!(r2.err().contains("multiple of 5"));
}

#[test]
fn base58_keeps_leading_zero_bytes() {
    let raw: &[u8] = &[0, 0, 1];
    let e = t::run_bytes("basenc", &[ "--base58", "-w", "0"], raw);
    assert_eq!(e.stdout, b"112");
    assert_eq!(t::run_bytes("basenc", &["-d",  "--base58", "-w", "0"], b"112").stdout, raw.to_vec());
}

#[test]
fn base58_alphabet_skips_ambiguous_chars() {
    // no 0, I, O in the alphabet
    let r = t::run("basenc", &["-d",  "--base58", "-w", "0"], "0IO");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid input"));
}

#[test]
fn ignore_garbage_skips_invalid_chars() {
    let r = t::run("basenc", &["-d", "-i",  "--base64", "-w", "0"], "aGVsbG8gd29ybGQK!x\n");
    assert_eq!(r.out(), "hello world\n");
}

#[test]
fn invalid_char_without_ignore_exit_1() {
    let r = t::run("basenc", &["-d",  "--base16", "-w", "0"], "68656Z6C6F\n");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid input"));
}

#[test]
fn wrap_and_no_wrap() {
    let long = "a".repeat(200) + "\n";
    let out = enc("--base64", &long).out();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0].len(), 76);
    assert_eq!(t::run("basenc", &[ "--base64", "-w", "0"], &long).out().lines().count(), 1);
    assert_eq!(t::run("basenc", &["--base64", "-w40"], &long).out().lines().count(), 7);
}

#[test]
fn missing_file_exit_1() {
    let r = t::run("basenc", &[ "--base64", "/definitely/not/here"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("basenc:"));
}

#[test]
fn unknown_option_and_missing_operand_exit_2() {
    assert_eq!(t::run("basenc", &["--nope"], "x").code, 2);
    assert_eq!(t::run("basenc", &["-w"], "x").code, 2);
    assert_eq!(t::run("basenc", &[ "--base99"], "x").code, 2);
}

#[test]
fn help_and_version() {
    assert!(t::run("basenc", &["--help"], "").out().contains("basenc"));
    assert!(t::run("basenc", &["--version"], "").out().contains("basenc"));
}

#[test]
fn e2e_binary_dispatch() {
    assert_eq!(
        t::run_bin("basenc", &[ "--base32", "-w", "0"], "hello world\n").out(),
        "NBSWY3DPEB3W64TMMQFA===="
    );
}

#[test]
fn fuzz_roundtrip_all_codecs() {
    let codecs = [
        "--base64",
        "--base64url",
        "--base32",
        "--base32hex",
        "--base16",
        "--base2lsbf",
        "--base2msbf",
        "--z85",
        "--base58",
    ];
    let mut rng = t::Rng::new(0x85E4);
    for _ in 0..60 {
        let len = rng.below(64) as usize;
        let data = rng.bytes(len);
        for codec in codecs {
            // z85 needs a multiple of 4 for encoding
            if codec == "--z85" && len % 4 != 0 {
                continue;
            }
            let e = t::run_bytes("basenc", &[codec, "-w", "0"], &data);
            assert_eq!(e.code, 0, "encode failed for {}", codec);
            let back = t::run_bytes("basenc", &["-d", codec, "-w", "0"], &e.stdout);
            assert_eq!(back.code, 0, "decode failed for {} on {:?}", codec, e.stdout);
            assert_eq!(back.stdout, data, "roundtrip failed for {} len {}", codec, len);
        }
    }
}

#[test]
fn fuzz_garbage_is_rejected_or_skipped() {
    let mut rng = t::Rng::new(0x6A85);
    for _ in 0..60 {
        let len = 1 + rng.below(40) as usize;
        let s = rng.bytes(len);
        let r = t::run_bytes("basenc", &["-d",  "--base64", "-w", "0"], &s);
        // either it decoded cleanly, or it reported invalid input; never a crash
        assert!(r.code == 0 || r.code == 1, "unexpected exit {}", r.code);
        if r.code == 1 {
            assert!(r.err().contains("invalid input"));
        }
        // with -i every input decodes
        let g = t::run_bytes("basenc", &["-d", "-i",  "--base64", "-w", "0"], &s);
        assert_eq!(g.code, 0);
        assert!(g.stdout.len() <= s.len());
    }
}
