//! chown：ownership 變更（guest 為 root，可成功 chown）、錯誤措辭、--reference、-R。
use coreutils::testutil as t;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

fn ids(p: &str) -> (u32, u32) {
    let m = std::fs::symlink_metadata(p).unwrap();
    (m.uid(), m.gid())
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
fn numeric_owner_success() {
    let d = "/tmp/chown_t1";
    setup(d);
    let p = mk(d, "f1");
    let r = t::run("chown", &["100:101", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(ids(&p), (100, 101));
    let r = t::run("chown", &["102", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let (u, g) = ids(&p);
    assert_eq!(u, 102);
    assert_eq!(g, 101, "僅 owner 時 group 不變");
    let r = t::run("chown", &[":103", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let (u, g) = ids(&p);
    assert_eq!(u, 102);
    assert_eq!(g, 103, "僅 group 時 owner 不變");
}

#[test]
fn empty_spec_is_noop() {
    let d = "/tmp/chown_t2";
    setup(d);
    let p = mk(d, "f1");
    let before = ids(&p);
    let r = t::run("chown", &["", &p], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(ids(&p), before);
    let r = t::run("chown", &["", "/tmp/chown_noexist_zz"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        "chown: cannot dereference '/tmp/chown_noexist_zz': No such file or directory"
    );
}

#[test]
fn error_wordings() {
    let d = "/tmp/chown_t3";
    setup(d);
    let r = t::run("chown", &["100", "/tmp/chown_noexist_zz"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        "chown: cannot dereference '/tmp/chown_noexist_zz': No such file or directory"
    );
    // guest 無 /etc/passwd 的 nobody：名稱解析先於 stat → invalid user
    let r = t::run("chown", &["-h", "100", "/tmp/chown_noexist_zz"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        "chown: cannot access '/tmp/chown_noexist_zz': No such file or directory"
    );
    let r = t::run("chown", &["nobody", "/tmp/chown_noexist_zz"], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid user: 'nobody'"), "{}", r.err());
    let r = t::run("chown", &["bogususer_zz", format!("{}/x", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), "chown: invalid user: 'bogususer_zz'");
    let p = mk(d, "f1");
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
    let r = t::run("chown", &["1:2:3", &p], "");
    assert_ne!(r.code, 0); // 多餘冒號段：非零即可（措辭不追隨 clap）
    // dangling symlink 預設追隨 → cannot dereference
    let sl = format!("{}/sl", d);
    std::os::unix::fs::symlink("/tmp/chown_dangling_zz", &sl).unwrap();
    let r = t::run("chown", &["100", &sl], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("cannot dereference"), "{}", r.err());
}

#[test]
fn reference_option() {
    let d = "/tmp/chown_t4";
    setup(d);
    let r1 = mk(d, "ref");
    let _ = t::run("chown", &["200:201", &r1], "");
    let f = mk(d, "f");
    let r = t::run("chown", &["--reference", &r1, &f], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(ids(&f), (200, 201));
    // 有 --reference 時 positional 全是 FILE
    let r = t::run("chown", &["--reference", &r1], "");
    assert_ne!(r.code, 0);
    let r = t::run("chown", &["--reference", "/tmp/chown_noexist_zz", &f], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        "chown: failed to get attributes of '/tmp/chown_noexist_zz': No such file or directory"
    );
}

#[test]
fn recursive_verbose_and_h() {
    let d = "/tmp/chown_t5";
    setup(d);
    std::fs::create_dir_all(format!("{}/sub/deep", d)).unwrap();
    let a = mk(d, "sub/a");
    let b = mk(d, "sub/deep/b");
    let r = t::run("chown", &["-R", "-v", "300:301", &format!("{}/sub", d)], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(ids(&format!("{}/sub", d)), (300, 301));
    assert_eq!(ids(&a), (300, 301));
    assert_eq!(ids(&b), (300, 301));
    let out = r.out();
    assert!(out.contains("changed ownership of"), "{}", out);
    // -h 對 symlink 本身 chown
    let sl = format!("{}/sl", d);
    let tgt = mk(d, "tgt");
    std::os::unix::fs::symlink(&tgt, &sl).unwrap();
    let _ = t::run("chown", &["400", &tgt], "");
    let r = t::run("chown", &["-h", "500", &sl], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let m = std::fs::symlink_metadata(&sl).unwrap();
    assert_eq!(m.uid(), 500);
    assert_eq!(ids(&tgt), (400, m_gid_of(&tgt)), "symlink chown 不影響目標");
}

fn m_gid_of(p: &str) -> u32 {
    std::fs::metadata(p).unwrap().gid()
}

#[test]
fn misc_options() {
    let r = t::run("chown", &[], "");
    assert_eq!(r.code, 2);
    let r = t::run("chown", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Usage"));
    let r = t::run("chown", &["--version"], "");
    assert_eq!((r.code, r.out().trim_end()), (0, "chown (uutils coreutils) 0.10.0"));
    let r = t::run("chown", &["-x", "0", "/tmp"], "");
    assert_eq!(r.code, 2);
}

#[test]
fn fuzz_numeric_roundtrip() {
    let d = "/tmp/chown_fz";
    setup(d);
    let mut rng = t::Rng::new(20261012);
    for i in 0..30 {
        let u = rng.below(70000) as u32;
        let g = rng.below(70000) as u32;
        let p = mk(d, &format!("f{}", i));
        let spec = format!("{}:{}", u, g);
        let r = t::run("chown", &[&spec, &p], "");
        assert_eq!(r.code, 0, "{}: {}", spec, r.err());
        assert_eq!(ids(&p), (u, g), "spec {}", spec);
    }
}
