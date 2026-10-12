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
