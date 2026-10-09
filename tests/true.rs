use coreutils::testutil::*;

#[test]
fn exits_zero() {
    let r = run("true", &[], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "");
}

#[test]
fn arguments_are_ignored() {
    let r = run("true", &["foo", "bar"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "");
}

#[test]
fn help_and_version_print_text() {
    let h = run("true", &["--help"], "");
    assert_eq!(h.code, 0);
    assert!(h.out().contains("true"), "help must name the applet: {}", h.out());
    let v = run("true", &["--version"], "");
    assert_eq!(v.code, 0);
    assert!(v.out().contains("true"), "version must name the applet: {}", v.out());
}

#[test]
fn e2e_binary_dispatch() {
    let r = run_bin("true", &[], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "");
}

#[test]
fn fuzz_any_args_still_exit_0() {
    // man page：「ignored command line arguments」→ 任何非選項參數都不影響結果
    let mut rng = Rng::new(0x71BE);
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
        let r = run("true", &refs, "");
        assert_eq!(r.code, 0, "args={args:?}");
        assert_eq!(r.out(), "", "args={args:?}");
    }
}
