use coreutils::testutil::*;

#[test]
fn always_exits_1() {
    let r = run("false", &[], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.out(), "");
}

#[test]
fn arguments_do_not_change_the_status() {
    let r = run("false", &["foo", "bar"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.out(), "");
}

#[test]
fn help_still_returns_1() {
    let h = run("false", &["--help"], "");
    assert_eq!(h.code, 1);
    assert!(h.out().contains("false"), "help must name the applet: {}", h.out());
    let v = run("false", &["--version"], "");
    assert_eq!(v.code, 1);
    assert!(v.out().contains("false"), "version must name the applet: {}", v.out());
}

#[test]
fn io_error_is_diagnosed_but_status_is_still_1() {
    // man page：「Any IO error during this operation is diagnosed, yet the program will also return 1.」
    let mut w = FailWriter::new(0);
    let mut e = Vec::new();
    let code = run_with("false", &["--help"], &[], &mut w, &mut e);
    assert_eq!(code, 1);
    assert!(!e.is_empty(), "IO error must be diagnosed on stderr");
    let mut w2 = FailWriter::new(0);
    let mut e2 = Vec::new();
    let code2 = run_with("false", &["--version"], &[], &mut w2, &mut e2);
    assert_eq!(code2, 1);
    assert!(!e2.is_empty());
}

#[test]
fn e2e_binary_dispatch() {
    let r = run_bin("false", &[], "");
    assert_eq!(r.code, 1);
}

#[test]
fn fuzz_status_is_always_1() {
    // man page 不變量：不論參數為何，exit status 恆為 1
    let mut rng = Rng::new(0xFA5E);
    for _ in 0..200 {
        let n = rng.below(5) as usize;
        let mut args: Vec<String> = Vec::new();
        for _ in 0..n {
            let len = 1 + rng.below(10) as usize;
            let bytes = rng.bytes(len);
            let s = String::from_utf8_lossy(&bytes).into_owned();
            if s == "--help" || s == "--version" {
                continue;
            }
            args.push(s);
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let r = run("false", &refs, "");
        assert_eq!(r.code, 1, "args={args:?}");
    }
}
