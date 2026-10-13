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

#[test]
fn fuzz_invariants() {
    // 不變量：whoami 的輸出恆為單一 token（有效使用者名）、rc 0；任意無效旗標 rc 2；
    // 任何選項組合（-h/-V 除外）不改變身份輸出。
    // 同 binary 的其他測試會併發增刪 /etc/passwd；先寫回 root 並重試以避開競態。
    let mut base = run(&[]);
    for _ in 0..4 {
        if base.code == 0 {
            break;
        }
        std::fs::create_dir_all("/etc").unwrap();
        std::fs::write("/etc/passwd", "root:x:0:0:root:/root:/bin/sh\n").unwrap();
        base = run(&[]);
    }
    assert_eq!(base.code, 0, "{}", base.err());
    let name = base.out().trim().to_string();
    assert!(!name.is_empty() && !name.contains(' '), "{name}");
    let mut st: u64 = 0xF10A_1E1E;
    for _ in 0..60 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let pool = ["--help", "--version", "-h", "-V", "--", "x", "-q", "--bogus", "-z"];
        let a1 = pool[((st >> 30) % 9) as usize];
        let a2 = pool[((st >> 20) % 9) as usize];
        let r = run(&[a1, a2]);
        match r.code {
            0 => {
                // help/version 或純身份輸出
                let o = r.out();
                assert!(
                    o.trim() == name || o.contains("Usage: whoami") || o.starts_with("whoami (uutils"),
                    "{a1} {a2}: {o:?}"
                );
            }
            2 => {
                assert_eq!(r.out(), "", "{a1} {a2}");
                assert!(r.err().starts_with("error: "), "{a1} {a2}: {:?}", r.err());
            }
            1 => {
                // whoami 家族：unknown/operand → rc1，零 stdout
                assert_eq!(r.out(), "", "{a1} {a2}: {}", r.err());
            }
            c => panic!("unexpected rc {c} for {a1} {a2}"),
        }
    }
    // 有效長旗標前綴不改變輸出（我方 resolve_long 慣例）
    let r = run(&["--vers"]);
    assert_eq!(r.code, 0);
    assert!(r.out().starts_with("whoami (uutils"), "{}", r.out());
}
