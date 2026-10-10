//! link/unlink guest 測試：措辭與 rc。

use coreutils::testutil as t;
use std::os::unix::fs::MetadataExt;

fn fresh(tag: &str) -> std::path::PathBuf {
    let d = std::path::PathBuf::from(format!("/tmp/lkt_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::env::set_current_dir(&d).unwrap();
    d
}

#[test]
fn link_basic_and_errors() {
    fresh("link");
    std::fs::write("src1", b"hi").unwrap();
    let r = t::run("link", &["src1", "l1"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    assert_eq!(std::fs::read("l1").unwrap(), b"hi");
    // 兩hard link 同 inode
    let a = std::fs::metadata("src1").unwrap();
    let b = std::fs::metadata("l1").unwrap();
    use std::os::unix::fs::MetadataExt;
    assert_eq!((a.ino(), a.nlink()), (b.ino(), 2));
    let r = t::run("link", &["src1", "l1"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "link: cannot create link 'l1' to 'src1': Already exists"));
    let r = t::run("link", &["noexist_zz", "l9"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "link: cannot create link 'l9' to 'noexist_zz': No such file or directory"));
    // EISDIR 走 io_reason（無後綴）
    std::fs::create_dir("dir1").unwrap();
    let r = t::run("link", &["dir1", "dl"], "");
    assert_eq!((r.code, r.err().contains("link: cannot create link 'dl' to 'dir1':")), (1, true));
    // operand 數目
    for args in [vec!["src1"], vec!["src1", "a", "b"], vec![]] {
        let r = t::run("link", &args, "");
        assert_eq!((r.code, r.err().contains("missing file operand")), (2, true));
    }
    let r = t::run("link", &["--help"], "");
    assert_eq!((r.code, r.out().contains("Usage: link")), (0, true));
    let r = t::run("link", &["--version"], "");
    assert_eq!((r.code, r.out().trim_end()), (0, "link (uutils coreutils) 0.10.0"));
    let r = t::run("link", &["-V"], "");
    assert_eq!((r.code, r.out().contains("uutils")), (0, true));
    let r = t::run("link", &["--", "src1", "dashlink"], "");
    assert_eq!(r.code, 0);
}


#[test]
fn fuzz_link_unlink_roundtrip() {
    fresh("fz");
    let mut rng = t::Rng::new(990011);
    std::fs::write("src", b"data").unwrap();
    for i in 0..50 {
        let name = format!("h{}", rng.below(100000));
        let r = t::run("link", &["src", &name], "");
        assert_eq!(r.code, 0, "case {i}: {}", r.err());
        let content = std::fs::read(&name).unwrap();
        assert_eq!(content, b"data");
        let r = t::run("unlink", &[&name], "");
        assert_eq!(r.code, 0);
        assert!(!std::path::Path::new(&name).exists());
    }
    // src 未被 unlink 掉
    assert_eq!(std::fs::metadata("src").unwrap().nlink(), 1);
}
