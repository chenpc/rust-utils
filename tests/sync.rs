//! sync 測試：guest 以 root 執行，全部走 in-process dispatch。

use coreutils::testutil as t;

#[test]
fn no_args_syncs_all() {
    let r = t::run("sync", &[], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, b"");
    assert_eq!(r.stderr, b"");
}

#[test]
fn syncs_regular_file() {
    let dir = "/tmp/sc";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/f1"), b"data").unwrap();
    let r = t::run("sync", &[&format!("{dir}/f1")], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.stderr, b"");
}

#[test]
fn data_only_mode() {
    let dir = "/tmp/sc";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/f1"), b"data").unwrap();
    let r = t::run("sync", &["-d", &format!("{dir}/f1")], "");
    assert_eq!(r.code, 0);
    // --data 同義
    let r2 = t::run("sync", &["--data", &format!("{dir}/f1")], "");
    assert_eq!(r2.code, 0);
}

#[test]
fn file_system_mode() {
    let r = t::run("sync", &["-f"], "");
    assert_eq!(r.code, 0);
    let r2 = t::run("sync", &["--file-system"], "");
    assert_eq!(r2.code, 0);
    // 帶檔案也接受
    let dir = "/tmp/sc";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/f1"), b"data").unwrap();
    let r3 = t::run("sync", &["-f", &format!("{dir}/f1")], "");
    assert_eq!(r3.code, 0);
}

#[test]
fn multiple_files_one_bad() {
    let dir = "/tmp/sc";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/f1"), b"data").unwrap();
    let r = t::run("sync", &[&format!("{dir}/f1"), "/tmp/sc/noexist_zz"], "");
    assert_eq!(r.code, 1);
    assert!(
        r.stderr
            .starts_with(b"sync: error opening '/tmp/sc/noexist_zz': No such file or directory\n"),
        "{:?}",
        String::from_utf8_lossy(&r.stderr)
    );
}

#[test]
fn directory_operand_ok() {
    // 目錄 fsync 在 Linux 合法
    std::fs::create_dir_all("/tmp/sc").unwrap();
    let r = t::run("sync", &["/tmp/sc"], "");
    assert_eq!(r.code, 0);
}

#[test]
fn conflict_fs_data() {
    let dir = "/tmp/sc";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/f1"), b"data").unwrap();
    let r = t::run("sync", &["-f", "-d", &format!("{dir}/f1")], "");
    assert_eq!(r.code, 1);
    assert!(
        r.stderr
            .starts_with(b"sync: the argument '--file-system' cannot be used with '--data'"),
        "{:?}",
        String::from_utf8_lossy(&r.stderr)
    );
}

#[test]
fn data_without_file_errors() {
    let r = t::run("sync", &["-d"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.stderr,
        b"sync: --data needs at least one argument\n"
    );
}

#[test]
fn unrecognized_option() {
    // 我方自訂解析採 GNU 慣例 rc=2；參考（clap）措辭與 rc 不追隨
    let r = t::run("sync", &["-x"], "");
    assert_eq!(r.code, 2);
    assert!(r.stderr.starts_with(b"sync: invalid option -- 'x'"));
    let r2 = t::run("sync", &["--bogus"], "");
    assert_eq!(r2.code, 2);
    assert!(r2.stderr.starts_with(b"sync: unrecognized option '--bogus'"));
}

#[test]
fn help_and_version() {
    let r = t::run("sync", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.stdout.starts_with(b"Usage: sync"));
    let r = t::run("sync", &["-h"], "");
    assert_eq!(r.code, 0);
    let r = t::run("sync", &["--version"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, b"sync (uutils coreutils) 0.10.0\n");
    let r = t::run("sync", &["-V"], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.stdout, b"sync (uutils coreutils) 0.10.0\n");
}

#[test]
fn dash_dash_terminates_options() {
    let dir = "/tmp/sc";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(format!("{dir}/-weird"), b"x").unwrap();
    let r = t::run("sync", &["--", &format!("{dir}/-weird")], "");
    assert_eq!(r.code, 0);
}
