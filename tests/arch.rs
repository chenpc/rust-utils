use coreutils::testutil as t;

#[test]
fn prints_machine_name() {
    let r = t::run("arch", &[], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "x86_64\n");
}

#[test]
fn help_and_version_are_verbatim() {
    let r = t::run("arch", &["--help"], "");
    assert_eq!(
        r.out(),
        "Display machine architecture\n\nUsage: arch\n\nOptions:\n  -h, --help     Print help\n  -V, --version  Print version\n\nDetermine architecture name for current machine.\n"
    );
    let r = t::run("arch", &["--version"], "");
    assert_eq!(r.out(), "arch (uutils coreutils) 0.10.0\n");
    assert_eq!(t::run("arch", &["-h"], "").out(), r_out_help());
    assert_eq!(t::run("arch", &["-V"], "").out(), "arch (uutils coreutils) 0.10.0\n");
    assert_eq!(t::run("arch", &["-hV"], "").out(), r_out_help());
    assert_eq!(t::run("arch", &["-Vh"], "").out(), "arch (uutils coreutils) 0.10.0\n");
}

fn r_out_help() -> String {
    t::run("arch", &["--help"], "").out()
}

#[test]
fn any_operand_or_unknown_option_is_a_clap_error() {
    let want = "error: unexpected argument 'x' found\n\nUsage: arch\n\nFor more information, try '--help'.\n";
    assert_eq!(t::run("arch", &["x"], "").err(), want);
    assert_eq!(t::run("arch", &["x"], "").code, 1);
    let want = "error: unexpected argument '--bogus' found\n\nUsage: arch\n\nFor more information, try '--help'.\n";
    assert_eq!(t::run("arch", &["--bogus"], "").err(), want);
    let want = "error: unexpected argument '-x' found\n\nUsage: arch\n\nFor more information, try '--help'.\n";
    assert_eq!(t::run("arch", &["-x"], "").err(), want);
}

#[test]
fn long_option_prefix_inference_and_value_errors() {
    // clap infers a unique long-option prefix.
    let want = "error: unexpected argument '--bogus' found\n\nUsage: arch\n\nFor more information, try '--help'.\n";
    let r = t::run("arch", &["--hel"], "");
    assert_eq!(r.code, 0, "err {:?}", r.err());
    assert_eq!(r.out(), t::run("arch", &["--help"], "").out());
    let r = t::run("arch", &["--vers"], "");
    assert_eq!(r.out(), "arch (uutils coreutils) 0.10.0\n");
    assert_eq!(t::run("arch", &["--bogus"], "").err(), want);
    // A flag that takes no value rejects an inline one.
    let want = "error: unexpected value '1' for '--help' found; no more were expected\n\nUsage: arch\n\nFor more information, try '--help'.\n";
    let r = t::run("arch", &["--help=1"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), want);
    let want = "error: unexpected value '1' for '--version' found; no more were expected\n\nUsage: arch\n\nFor more information, try '--help'.\n";
    assert_eq!(t::run("arch", &["--version=1"], "").err(), want);
    // `--` ends option parsing; anything after it is an operand and arch has none.
    let r = t::run("arch", &["--"], "");
    assert_eq!(r.code, 0, "err {:?}", r.err());
    assert_eq!(r.out(), "x86_64\n");
    let r = t::run("arch", &["--", "--bogus"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: unexpected argument '--bogus' found\n\nUsage: arch\n\nFor more information, try '--help'.\n"
    );
}

#[test]
fn run_bin_dispatch() {
    let out = t::run_bin("arch", &[], "");
    assert_eq!(out.code, 0);
    assert_eq!(out.out(), "x86_64\n");
}

#[test]
fn fuzz_invariants() {
    // 不變量：arch 成功路徑恆印同一 machine 字串（uname -m 語義，不因旗標而變）；
    // rc ∈ {0,2}；rc2 恆零輸出。
    let base = t::run("arch", &[], "");
    assert_eq!(base.code, 0);
    let machine = base.out().trim().to_string();
    assert!(!machine.is_empty() && !machine.contains(' '));
    let mut st: u64 = 0xAAC4_15F0;
    for _ in 0..50 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let pool = ["--help", "-h", "--version", "-V", "--", "junk", "--hel", "--vers", "-a"];
        let a1 = pool[((st >> 30) % 9) as usize];
        let a2 = pool[((st >> 20) % 9) as usize];
        let r = t::run("arch", &[a1, a2], "");
        assert!(matches!(r.code, 0 | 1 | 2), "{a1} {a2} -> {}", r.code); // 家族慣例：operand/未知旗標 rc1
        if r.code == 0 {
            let o = r.out().trim().to_string();
            assert!(
                o == machine || o.contains("Usage: arch") || o.starts_with("arch (uutils"),
                "{a1} {a2}: {o:?}"
            );
        } else {
            assert_eq!(r.out(), "", "{a1} {a2}");
            assert!(r.err().starts_with("error: "), "{a1} {a2}: {:?}", r.err());
        }
    }
}
