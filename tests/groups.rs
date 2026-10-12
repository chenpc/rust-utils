//! groups(1)：印出群組名稱（man：「Print the group names and IDs, given each
//! NAME or the current process' real-or-effective group ID」）。

use coreutils::testutil as t;

fn run(args: &[&str]) -> t::Out {
    t::run("groups", args, "")
}

fn clear_db() {
    let _ = std::fs::remove_file("/etc/passwd");
    let _ = std::fs::remove_file("/etc/group");
}

fn set_db(passwd: &str, group: &str) {
    clear_db();
    std::fs::create_dir_all("/etc").unwrap();
    std::fs::write("/etc/passwd", passwd).unwrap();
    std::fs::write("/etc/group", group).unwrap();
}

/// 無 operand、無 /etc/group：印數字且每個失敗都印 warning（本機參考在無 db 沙箱的實測）。
#[test]
fn current_groups_without_database() {
    clear_db();
    let r = run(&[]);
    assert_eq!((r.code, r.out().as_str()), (1, "0\n"));
    assert_eq!(r.err(), "groups: cannot find name for group ID 0\n");
}

#[test]
fn current_groups_with_database() {
    set_db("root:x:0:0:root:/root:/bin/sh\n", "root:x:0:\n");
    let r = run(&[]);
    assert_eq!((r.code, r.out().as_str()), (0, "root\n"));
    assert_eq!(r.err(), "");
    clear_db();
}

/// 有 operand 時標籤是 operand 原文（實測 `groups 0` → `0 : <名稱>`）。
#[test]
fn operand_label_is_the_original_text() {
    set_db("root:x:0:0:root:/root:/bin/sh\nu1:x:1000:100:U1:/home/u1:/bin/sh\n", "root:x:0:\nga:x:100:\n");
    let r = run(&["0"]);
    assert_eq!((r.code, r.out().as_str()), (0, "0 : root\n"));
    let r = run(&["root"]);
    assert_eq!((r.code, r.out().as_str()), (0, "root : root\n"));
    let r = run(&["u1"]);
    assert_eq!((r.code, r.out().as_str()), (0, "u1 : ga\n"));
    clear_db();
}

#[test]
fn multiple_operands_print_one_line_each() {
    set_db("root:x:0:0:root:/root:/bin/sh\nbin:x:2:2:bin:/bin:/bin/sh\n", "root:x:0:\nbin:x:2:\n");
    let r = run(&["root", "bin"]);
    assert_eq!((r.code, r.out().as_str()), (0, "root : root\nbin : bin\n"));
    clear_db();
}

#[test]
fn unknown_operand_is_reported() {
    clear_db();
    let r = run(&["nosuchuser"]);
    assert_eq!((r.code, r.out().as_str()), (1, ""));
    assert_eq!(r.err(), "groups: 'nosuchuser': no such user\n");

    let r = run(&[""]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "groups: '': no such user\n");

    let r = run(&["a'b"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "groups: \"a'b\": no such user\n");

    // `--` 之後一律是 operand
    let r = run(&["--", "-x"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "groups: '-x': no such user\n");
}

/// 參考對 unknown 選項有 tip 行（groups 有 positional → clap 提示 `-- <arg>`）。
#[test]
fn clap_style_errors() {
    let r = run(&["-x"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: unexpected argument '-x' found\n\n  tip: to pass '-x' as a value, use '-- -x'\n\nUsage: groups [OPTION]... [USERNAME]...\n\nFor more information, try '--help'.\n"
    );
    let r = run(&["--bogus"]);
    assert_eq!(r.code, 1);
    assert!(r.err().starts_with("error: unexpected argument '--bogus' found\n\n"));
}

#[test]
fn help_and_version_are_verbatim() {
    let r = run(&["--help"]);
    assert_eq!(r.code, 0);
    let help = "Print group memberships for each USERNAME or, if no USERNAME is specified, for
the current process (which may differ if the groups data\u{2010}base has changed).

Usage: groups [OPTION]... [USERNAME]...

Arguments:
  [USERNAME]...  

Options:
  -h, --help     Print help
  -V, --version  Print version\n";
    assert_eq!(r.out(), help);
    let r = run(&["--version"]);
    assert_eq!((r.code, r.out().as_str()), (0, "groups (uutils coreutils) 0.10.0\n"));
    assert_eq!(t::run("groups", &["-hV"], "").out(), help);
    assert_eq!(
        t::run("groups", &["-V"], "").out(),
        "groups (uutils coreutils) 0.10.0\n"
    );
}

#[test]
fn e2e_binary_dispatch() {
    let r = t::run_bin("groups", &[], "");
    assert_eq!(r.code, 1);
    assert!(r.out().chars().all(|c| c.is_ascii_whitespace() || c.is_ascii_digit()));
}

#[test]
fn fuzz_operands_and_flags() {
    clear_db();
    let mut rng = t::Rng::new(20261127);
    let operands = ["0", "1", "root", "bin", "nobody", "nosuch", "", "--", "1000"];
    for _ in 0..12 {
        let k = rng.below(3) as usize;
        let mut args: Vec<String> = Vec::new();
        for _ in 0..k {
            args.push(operands[rng.below(operands.len() as u64) as usize].to_string());
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let a = run(&refs);
        let b = run(&refs);
        assert_eq!((a.code, a.out(), a.err()), (b.code, b.out(), b.err()), "args {:?}", refs);
        assert!(a.code == 0 || a.code == 1, "args {:?}", refs);
    }
}
