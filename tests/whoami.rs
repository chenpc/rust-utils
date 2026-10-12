//! whoami(1)：印出目前程序有效使用者名稱（man：「Print the effective user name」）。
//! 本專案依本機參考：由 geteuid() 查 /etc/passwd；不受 LOGNAME 環境影響（實測）。

use coreutils::testutil as t;

fn run(args: &[&str]) -> t::Out {
    t::run("whoami", args, "")
}

fn clear_db() {
    let _ = std::fs::remove_file("/etc/passwd");
    let _ = std::fs::remove_file("/etc/group");
}

#[test]
fn resolves_effective_user_from_passwd() {
    std::fs::create_dir_all("/etc").unwrap();
    std::fs::write("/etc/passwd", "root:x:0:0:root:/root:/bin/sh\n").unwrap();
    let _ = std::fs::remove_file("/etc/group");
    let r = run(&[]);
    assert_eq!((r.code, r.out().as_str()), (0, "root\n"));
    assert_eq!(r.err(), "");
}

/// 無 /etc/passwd → 參考措辭 `whoami: failed to get username: No such id: <uid>`、rc 1。
#[test]
fn missing_database_is_an_error() {
    clear_db();
    let r = run(&[]);
    assert_eq!((r.code, r.out().as_str()), (1, ""));
    assert_eq!(r.err(), "whoami: failed to get username: No such id: 0\n");
}

/// LOGNAME 環境不影響結果（實測：`env LOGNAME=zzz whoami` 仍印 db 名稱）。
#[test]
fn environment_does_not_change_the_answer() {
    std::fs::create_dir_all("/etc").unwrap();
    std::fs::write("/etc/passwd", "root:x:0:0:root:/root:/bin/sh\n").unwrap();
    std::env::set_var("LOGNAME", "zzz");
    std::env::set_var("USER", "yyy");
    let r = run(&[]);
    assert_eq!((r.code, r.out().as_str()), (0, "root\n"));
    std::env::remove_var("LOGNAME");
    std::env::remove_var("USER");
    clear_db();
}

#[test]
fn no_operands_allowed() {
    let r = run(&["root"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: unexpected argument 'root' found\n\nUsage: whoami\n\nFor more information, try '--help'.\n"
    );
    let r = run(&["-x"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: unexpected argument '-x' found\n\nUsage: whoami\n\nFor more information, try '--help'.\n"
    );
    let r = run(&["--bogus"]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("Usage: whoami"));
}

#[test]
fn help_and_version_are_verbatim() {
    let r = run(&["--help"]);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out(),
        "Print the current username.

Usage: whoami

Options:
  -h, --help     Print help
  -V, --version  Print version\n"
    );
    assert_eq!(run(&["-h"]).out(), r.out());
    let r = run(&["--version"]);
    assert_eq!((r.code, r.out().as_str()), (0, "whoami (uutils coreutils) 0.10.0\n"));
    assert_eq!(
        t::run("whoami", &["-Vh"], "").out(),
        "whoami (uutils coreutils) 0.10.0\n"
    );
}

#[test]
fn e2e_binary_dispatch() {
    std::fs::create_dir_all("/etc").unwrap();
    std::fs::write("/etc/passwd", "root:x:0:0:root:/root:/bin/sh\n").unwrap();
    let r = t::run_bin("whoami", &[], "");
    assert_eq!((r.code, r.out().as_str()), (0, "root\n"));
    clear_db();
    let r = t::run_bin("whoami", &["--version"], "");
    assert_eq!((r.code, r.out().as_str()), (0, "whoami (uutils coreutils) 0.10.0\n"));
}
