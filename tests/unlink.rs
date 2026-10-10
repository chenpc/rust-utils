//! link/unlink guest 測試：措辭與 rc。

use coreutils::testutil as t;

fn fresh(tag: &str) -> std::path::PathBuf {
    let d = std::path::PathBuf::from(format!("/tmp/ukt_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::env::set_current_dir(&d).unwrap();
    d
}

#[test]
fn unlink_basic_and_errors() {
    fresh("unlink");
    std::fs::write("f1", b"x").unwrap();
    let r = t::run("unlink", &["f1"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    let r = t::run("unlink", &["f1"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "unlink: cannot unlink 'f1': No such file or directory"));
    // symlink 可删
    std::fs::write("t1", b"x").unwrap();
    std::os::unix::fs::symlink("t1", "sl").unwrap();
    let r = t::run("unlink", &["sl"], "");
    assert_eq!(r.code, 0);
    assert!(std::path::Path::new("t1").exists());
    // 目錄：unlink 不能删目錄
    std::fs::create_dir("d1").unwrap();
    let r = t::run("unlink", &["d1"], "");
    assert_eq!((r.code, r.err().contains("cannot unlink 'd1'")), (1, true));
    for args in [vec![], vec!["a", "b"]] {
        let r = t::run("unlink", &args, "");
        assert_eq!((r.code, r.err().contains("missing operand")), (2, true));
    }
    let r = t::run("unlink", &["--help"], "");
    assert_eq!((r.code, r.out().contains("Usage: unlink")), (0, true));
    let r = t::run("unlink", &["-V"], "");
    assert_eq!((r.code, r.out().trim_end()), (0, "unlink (uutils coreutils) 0.10.0"));
    let r = t::run("unlink", &["--bogus"], "");
    assert_eq!((r.code, r.err().contains("unrecognized option")), (2, true));
}

