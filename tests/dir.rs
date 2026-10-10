//! dir：ls 的別名（本機參考 uutils 0.10.0 的 dir 為 column 列表，措辭/旗標與 ls 相同）。
//! 判定用不變量（同一目錄集合、指示符、rc）。
use coreutils::testutil as t;

const D: &str = "/tmp/dir_t";

fn setup() {
    let _ = std::fs::remove_dir_all(D);
    std::fs::create_dir_all(format!("{}/sub", D)).unwrap();
    std::fs::write(format!("{}/f1", D), b"hello").unwrap();
    std::fs::write(format!("{}/a.txt", D), b"a").unwrap();
    std::os::unix::fs::symlink("f1", format!("{}/lnk", D)).unwrap();
    std::fs::write(format!("{}/.hid", D), b"h").unwrap();
}

fn dir_names(o: &str) -> Vec<String> {
    o.lines()
        .flat_map(|l| l.split_whitespace())
        .filter(|s| !s.starts_with("total"))
        .map(|s| s.trim_end_matches(|c| c == '/' || c == '@' || c == '*').to_string())
        .collect()
}

#[test]
fn dir_lists_same_set_as_ls_columns() {
    setup();
    let r = t::run("dir", &[D], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let ls = t::run("ls", &["-C", D], "");
    assert_eq!(ls.code, 0);
    let mut dset = dir_names(&r.out());
    dset.sort();
    dset.dedup();
    let mut lset = dir_names(&ls.out());
    lset.sort();
    lset.dedup();
    assert_eq!(dset, lset, "dir 與 ls column 集合相同：{:?}", dset);
    assert!(dset.contains(&"sub".to_string()) && dset.contains(&"a.txt".to_string()));
}

#[test]
fn dir_honours_common_flags() {
    setup();
    let ra = t::run("dir", &["-a", D], "");
    assert_eq!(ra.code, 0);
    let va = dir_names(&ra.out());
    assert!(va.contains(&".hid".to_string()), "-a 含隱藏檔：{:?}", va);
    assert!(va.contains(&".".to_string()) && va.contains(&"..".to_string()));
    let f = t::run("dir", &[D], "");
    let rf = t::run("dir", &["-r", D], "");
    assert_eq!(rf.code, 0);
    let mut a = dir_names(&f.out());
    let mut b = dir_names(&rf.out());
    a.sort();
    a.dedup();
    b.sort();
    b.dedup();
    assert_eq!(a, b, "-r 不改變集合");
    let r1 = t::run("dir", &["--help"], "");
    assert_eq!(r1.code, 0);
    assert!(r1.out().contains("Usage"), "{}", r1.out());
    let rbad = t::run("dir", &["--no-such-flag", D], "");
    assert_ne!(rbad.code, 0);
}

#[test]
fn dir_errors_like_ls() {
    setup();
    let r = t::run("dir", &["nonexistent"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("dir:"), "{}", r.err());
    assert!(r.err().contains("cannot access 'nonexistent'"), "{}", r.err());
    // -d：列 operand 本身
    let rd = t::run("dir", &["-d", D], "");
    assert_eq!(rd.code, 0);
    assert!(rd.out().contains(D), "-d 列 operand：{:?}", rd.out());
}
