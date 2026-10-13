//! hostid(1)：印出主機的數字識別碼（十六進位，man：「Print the numeric
//! identifier (in hexadecimal) for the current host」）。

use coreutils::testutil as t;

fn run(args: &[&str]) -> t::Out {
    t::run("hostid", args, "")
}

#[test]
fn prints_eight_hex_digits() {
    let r = run(&[]);
    assert_eq!(r.code, 0);
    assert_eq!(r.err(), "");
    let out = r.out();
    let line = out.trim_end();
    assert_eq!(line.len(), 8, "out {:?}", out);
    assert!(line.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    assert!(out.ends_with('\n'));
}

#[test]
fn no_operands_allowed() {
    for arg in ["123", "-x", "--bogus"] {
        let r = run(&[arg]);
        assert_eq!(r.code, 1, "arg {}", arg);
        assert_eq!(
            r.err(),
            format!(
                "error: unexpected argument '{}' found\n\nUsage: hostid [options]\n\nFor more information, try '--help'.\n",
                arg
            ),
            "arg {}",
            arg
        );
    }
}

#[test]
fn help_and_version_are_verbatim() {
    let r = run(&["--help"]);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out(),
        "Print the numeric identifier (in hexadecimal) for the current host

Usage: hostid [options]

Options:
  -h, --help     Print help
  -V, --version  Print version\n"
    );
    let r = run(&["--version"]);
    assert_eq!((r.code, r.out().as_str()), (0, "hostid (uutils coreutils) 0.10.0\n"));
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("hostid", &[], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.out().trim_end().len(), 8);
}

#[test]
fn fuzz_invariants() {
    // 不變量：hostid 輸出恆 8 位小寫 hex（任意旗標組合的成功路徑同一值）；rc ∈ {0,2}。
    let base = run(&[]);
    assert_eq!(base.code, 0);
    let v = base.out().trim().to_string();
    assert_eq!(v.len(), 8, "{v}");
    assert!(v.chars().all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()), "{v}");
    let mut st: u64 = 0x4057_1D0D;
    for _ in 0..50 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let pool = ["--help", "-h", "--version", "-V", "--", "junk", "--hel", "-z", "--verbose"];
        let a1 = pool[((st >> 30) % 9) as usize];
        let a2 = pool[((st >> 20) % 9) as usize];
        let r = run(&[a1, a2]);
        assert!(matches!(r.code, 0 | 1 | 2), "{a1} {a2} -> {}", r.code); // 家族慣例：operand/未知旗標 rc1
        if r.code == 0 {
            let o = r.out().trim().to_string();
            assert!(o == v || o.contains("Usage: hostid") || o.starts_with("hostid (uutils"), "{a1} {a2}: {o:?}");
        } else {
            assert_eq!(r.out(), "", "{a1} {a2}");
        }
    }
}
