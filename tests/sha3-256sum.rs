use coreutils::hashsum as h;
use coreutils::testutil as t;

const APPLET: &str = "sha3-256sum";
const ALGO: &h::Algo = &h::SHA3_256;

#[test]
fn output_matches_core_digest() {
    let input = b"hello world\n";
    let r = t::run_bytes(APPLET, &[], input);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), format!("{}  -\n", h::hex_of(&h::digest(ALGO, input, 0))));
}

#[test]
fn check_mode_round_trip() {
    let w = t::workdir();
    let f = w.join("in.bin");
    std::fs::write(&f, b"hello world\n").unwrap();
    let r = t::run(APPLET, &[f.to_str().unwrap()], "");
    assert_eq!(r.code, 0);
    let list = w.join("list.txt");
    std::fs::write(&list, &r.stdout).unwrap();
    let c = t::run(APPLET, &["-c", list.to_str().unwrap()], "");
    assert_eq!(c.code, 0);
    assert_eq!(c.out(), format!("{}: OK\n", f.to_str().unwrap()));
}

#[test]
fn fuzz_is_deterministic_and_self_checkable() {
    let mut rng = t::Rng::new(0xB0B0);
    let w = t::workdir();
    for i in 0..40 {
        let n = 1 + rng.below(200) as usize;
        let data = rng.bytes(n);
        let a = t::run_bytes(APPLET, &[], &data);
        let b = t::run_bytes(APPLET, &[], &data);
        assert_eq!(a.code, 0);
        assert_eq!(a.stdout, b.stdout, "not deterministic at iteration {}", i);
        assert_eq!(a.stdout.len(), ALGO.bits / 4 + 4, "hex length wrong at iteration {}", i);

        // 不變量：applet 自己印出的清單，必須能被自己的 -c 驗證通過
        let f = w.join(format!("fz{}.bin", i));
        std::fs::write(&f, &data).unwrap();
        let r = t::run(APPLET, &[f.to_str().unwrap()], "");
        assert_eq!(r.code, 0);
        assert_eq!(r.out(), format!("{}  {}\n", h::hex_of(&h::digest(ALGO, &data, 0)), f.to_str().unwrap()));
        let list = w.join(format!("fz{}.txt", i));
        std::fs::write(&list, &r.stdout).unwrap();
        let c = t::run(APPLET, &["-c", list.to_str().unwrap()], "");
        assert_eq!(c.code, 0, "self-check failed at iteration {}", i);
    }
}
