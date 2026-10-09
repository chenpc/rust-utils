use coreutils::hashsum as h;
use coreutils::testutil as t;

#[test]
fn requires_an_algorithm() {
    let r = t::run_bytes("hashsum", &["-"], b"hello world\n");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "hashsum: no algorithm specified\n");
}

#[test]
fn selector_last_wins() {
    let r = t::run_bytes("hashsum", &["--sha256", "--md5", "-"], b"hello world\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "6f5902ac237024bdd0c176cb93063dc4  -\n");
}

#[test]
fn no_names_omits_filename() {
    let r = t::run_bytes("hashsum", &["--no-names", "--sha256", "-"], b"hello world\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447\n");
}

#[test]
fn bits_only_for_shake() {
    let r = t::run_bytes("hashsum", &["--bits", "64", "--shake128", "-"], b"hello world\n");
    assert_eq!(r.code, 0);
    assert_eq!(r.out().len(), 16 + 4);
    let bad = t::run_bytes("hashsum", &["--bits", "64", "--sha256", "-"], b"hello world\n");
    assert_eq!(bad.code, 1);
}

#[test]
fn fuzz_selector_matches_core() {
    let mut rng = t::Rng::new(0x4A5B);
    for _ in 0..40 {
        let n = 1 + rng.below(200) as usize;
        let data = rng.bytes(n);
        for (sel, algo) in [("--md5", &h::MD5), ("--sha256", &h::SHA256), ("--b3sum", &h::BLAKE3), ("--shake256", &h::SHAKE256)] {
            let r = t::run_bytes("hashsum", &[sel, "-"], &data);
            assert_eq!(r.code, 0);
            assert_eq!(r.out(), format!("{}  -\n", h::hex_of(&h::digest(algo, &data, 0))));
        }
    }
}
