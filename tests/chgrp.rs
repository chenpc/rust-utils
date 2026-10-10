//! chgrp：group 變更（guest 為 root）、錯誤措辭用 "group"、--reference。
use coreutils::testutil as t;
use std::os::unix::fs::MetadataExt;

fn gid_of(p: &str) -> u32 {
    std::fs::symlink_metadata(p).unwrap().gid()
}

fn mk(d: &str, name: &str) -> String {
    let p = format!("{}/{}", d, name);
    std::fs::write(&p, b"x").unwrap();
    p
}

fn setup(d: &str) {
    let _ = std::fs::remove_dir_all(d);
    std::fs::create_dir_all(d).unwrap();
}

#[test]
fn numeric_group_success() {
    let d = "/tmp/chgrp_t1";
    setup(d);
    let p = mk(d, "f1");
    let before_uid = std::fs::metadata(&p).unwrap().uid();
    let r = t::run("chgrp", &["101", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(gid_of(&p), 101);
    assert_eq!(std::fs::metadata(&p).unwrap().uid(), before_uid, "owner 不變");
}

#[test]
fn error_wordings() {
    let r = t::run("chgrp", &["bogusgroup_zz", "/tmp"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), "chgrp: invalid group: 'bogusgroup_zz'");
    let r = t::run("chgrp", &["100", "/tmp/chgrp_noexist_zz"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        "chgrp: cannot dereference '/tmp/chgrp_noexist_zz': No such file or directory"
    );
    let r = t::run("chgrp", &["-h", "100", "/tmp/chgrp_noexist_zz"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        "chgrp: cannot access '/tmp/chgrp_noexist_zz': No such file or directory"
    );
}

#[test]
fn reference_and_recursive() {
    let d = "/tmp/chgrp_t2";
    setup(d);
    let r1 = mk(d, "ref");
    let _ = t::run("chgrp", &["210", &r1], "");
    let f = mk(d, "f");
    let r = t::run("chgrp", &["--reference", &r1, &f], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(gid_of(&f), 210);
    std::fs::create_dir_all(format!("{}/sub", d)).unwrap();
    let g = mk(d, "sub/g");
    let r = t::run("chgrp", &["-R", "211", &format!("{}/sub", d)], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(gid_of(&g), 211);
    assert_eq!(gid_of(&format!("{}/sub", d)), 211);
    // verbose 用 "group" 措辭
    let r = t::run("chgrp", &["-c", "212", &f], "");
    assert_eq!(r.code, 0, "{}", r.err());
}

#[test]
fn misc_and_fuzz() {
    let r = t::run("chgrp", &[], "");
    assert_eq!(r.code, 2);
    let r = t::run("chgrp", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Usage"));
    let r = t::run("chgrp", &["--version"], "");
    assert_eq!((r.code, r.out().trim_end()), (0, "chgrp (uutils coreutils) 0.10.0"));
    let r = t::run("chgrp", &["-x", "0", "/tmp"], "");
    assert_eq!(r.code, 2);
    let d = "/tmp/chgrp_fz";
    setup(d);
    let mut rng = t::Rng::new(20261013);
    for i in 0..20 {
        let g = rng.below(70000) as u32;
        let p = mk(d, &format!("f{}", i));
        let spec = g.to_string();
        let r = t::run("chgrp", &[&spec, &p], "");
        assert_eq!(r.code, 0, "{}: {}", spec, r.err());
        assert_eq!(gid_of(&p), g, "spec {}", spec);
    }
}
