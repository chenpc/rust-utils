//! logname(1)：印出登入名稱（man：「Print the user's login name」；
//! 本專案依參考：getlogin(3) 優先，失败時退回由 uid 查 /etc/passwd）。

use coreutils::testutil as t;

fn run(args: &[&str]) -> t::Out {
    t::run("logname", args, "")
}

fn clear_db() {
    let _ = std::fs::remove_file("/etc/passwd");
    let _ = std::fs::remove_file("/etc/group");
}

/// guest 無 utmp（getlogin 失敗）→ 退回 /etc/passwd 查詢（實測沙箱行為）。
#[test]
fn falls_back_to_passwd_lookup() {
    std::fs::create_dir_all("/etc").unwrap();
    std::fs::write("/etc/passwd", "root:x:0:0:root:/root:/bin/sh\n").unwrap();
    let _ = std::fs::remove_file("/etc/group");
    let r = run(&[]);
    assert_eq!((r.code, r.out().as_str()), (0, "root\n"));
    assert_eq!(r.err(), "");
}

#[test]
fn no_login_name_is_an_error() {
    clear_db();
    let r = run(&[]);
    assert_eq!((r.code, r.out().as_str()), (1, ""));
    assert_eq!(r.err(), "logname: no login name\n");
}

/// LOGNAME 環境不影響結果（man 未載，但實測兩邊一致）。
#[test]
fn environment_does_not_change_the_answer() {
    std::fs::create_dir_all("/etc").unwrap();
    std::fs::write("/etc/passwd", "root:x:0:0:root:/root:/bin/sh\n").unwrap();
    std::env::set_var("LOGNAME", "zzz");
    let r = run(&[]);
    assert_eq!((r.code, r.out().as_str()), (0, "root\n"));
    std::env::remove_var("LOGNAME");
    clear_db();
}

#[test]
fn no_operands_allowed() {
    for arg in ["root", "-x", "--bogus"] {
        let r = run(&[arg]);
        assert_eq!(r.code, 1, "arg {}", arg);
        assert_eq!(
            r.err(),
            format!("error: unexpected argument '{}' found\n\nUsage: logname\n\nFor more information, try '--help'.\n", arg),
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
        "Print user's login name

Usage: logname

Options:
  -h, --help     Print help
  -V, --version  Print version\n"
    );
    let r = run(&["--version"]);
    assert_eq!((r.code, r.out().as_str()), (0, "logname (uutils coreutils) 0.10.0\n"));
}

#[test]
fn e2e_binary_dispatch() {
    std::fs::create_dir_all("/etc").unwrap();
    std::fs::write("/etc/passwd", "root:x:0:0:root:/root:/bin/sh\n").unwrap();
    let r = t::run_bin("logname", &[], "");
    assert_eq!((r.code, r.out().as_str()), (0, "root\n"));
    clear_db();
}
