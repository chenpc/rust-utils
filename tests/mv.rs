//! mv applet e2e tests (guest).
use coreutils::testutil as t;
use std::fs;

fn fx(dir: &str) {
    let _ = fs::remove_dir_all(dir);
    fs::create_dir_all(format!("{}/d1/sub", dir)).unwrap();
    fs::write(format!("{}/f1", dir), b"hello").unwrap();
    fs::write(format!("{}/d1/f2", dir), b"two").unwrap();
    fs::write(format!("{}/d1/sub/f3", dir), b"three").unwrap();
    std::os::unix::fs::symlink("f1", format!("{}/sl", dir)).unwrap();
}

#[test]
fn rename_basics() {
    let d = "/tmp/mvt_basic";
    fx(d);
    let r = t::run("mv", &[format!("{}/f1", d).as_str(), format!("{}/g1", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/g1", d)).unwrap(), b"hello");
    assert!(!fs::metadata(format!("{}/f1", d)).is_ok());
    // same file
    let r = t::run("mv", &[format!("{}/g1", d).as_str(), format!("{}/g1", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("are the same file"), "{}", r.err());
    // into own subdirectory
    let r = t::run("mv", &[format!("{}/d1", d).as_str(), format!("{}/d1/d1", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("subdirectory of itself"), "{}", r.err());
    // missing source
    let r = t::run("mv", &[format!("{}/noexist_zz", d).as_str(), format!("{}/x", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("cannot stat"), "{}", r.err());
    // into directory (basename appended)
    let r = t::run("mv", &[format!("{}/g1", d).as_str(), format!("{}/d1", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/d1/g1", d)).unwrap(), b"hello");
    let _ = fs::remove_dir_all(d);
}

#[test]
fn verbose_symlink_and_dir_target() {
    let d = "/tmp/mvt_v";
    fx(d);
    let r = t::run("mv", &["-v", format!("{}/f1", d).as_str(), format!("{}/g1", d).as_str()], "");
    assert!(r.err().contains("renamed '") && r.err().contains("' -> '"), "{}", r.err());
    // symlink itself moves (readlink preserved)
    let r = t::run("mv", &[format!("{}/sl", d).as_str(), format!("{}/sl2", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read_link(format!("{}/sl2", d)).unwrap().to_string_lossy(), "f1");
    // non-empty dir onto existing dir → ENOTEMPTY wording
    let r = t::run("mv", &[format!("{}/d1", d).as_str(), format!("{}/sl2", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("cannot move"), "{}", r.err());
    // -T onto directory: rename dir to new name
    let r = t::run("mv", &["-T", format!("{}/d1", d).as_str(), format!("{}/dd", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/dd/sub/f3", d)).unwrap(), b"three");
    // dest with trailing slash but not a directory → failed to access (ref wording)
    let r = t::run("mv", &[format!("{}/sl2", d).as_str(), format!("{}/g1/", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("failed to access"), "{}", r.err());
    // multi operand, last not dir → one target error
    let r = t::run(
        "mv",
        &[format!("{}/sl2", d).as_str(), format!("{}/dd", d).as_str(), format!("{}/g1", d).as_str()],
        "",
    );
    assert_eq!(r.code, 1);
    assert!(r.err().contains("target '") && r.err().contains("Not a directory"), "{}", r.err());
    assert_eq!(r.err().matches("Not a directory").count(), 1, "{}", r.err());
    let _ = fs::remove_dir_all(d);
}

#[test]
fn clobber_update_interactive_backup() {
    let d = "/tmp/mvt_clob";
    fx(d);
    fs::write(format!("{}/old", d), b"OLD").unwrap();
    // -n: existing dest kept
    let r = t::run("mv", &["-n", format!("{}/f1", d).as_str(), format!("{}/old", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/old", d)).unwrap(), b"OLD");
    assert!(fs::metadata(format!("{}/f1", d)).is_ok());
    // -i decline (input n) → mv exits nonzero, file kept
    let r = t::run("mv", &["-i", format!("{}/f1", d).as_str(), format!("{}/old", d).as_str()], "n\n");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("overwrite"), "{}", r.err());
    assert_eq!(fs::read(format!("{}/old", d)).unwrap(), b"OLD");
    // -i accept
    let r = t::run("mv", &["-i", format!("{}/f1", d).as_str(), format!("{}/old", d).as_str()], "y\n");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/old", d)).unwrap(), b"hello");
    // backup with custom suffix, verbose mentions backup
    fs::write(format!("{}/f1", d), b"new").unwrap();
    fs::write(format!("{}/old", d), b"prev").unwrap();
    let r = t::run(
        "mv",
        &["-v", "--backup", "-S", ".bak", format!("{}/f1", d).as_str(), format!("{}/old", d).as_str()],
        "",
    );
    assert_eq!(r.code, 0);
    assert!(r.err().contains("(backup: '") && r.err().contains("old.bak'"), "{}", r.err());
    assert_eq!(fs::read(format!("{}/old.bak", d)).unwrap(), b"prev");
    assert_eq!(fs::read(format!("{}/old", d)).unwrap(), b"new");
    // hardlinked src/dst with backup allowed (ref renames dest away first)
    fs::write(format!("{}/hl", d), b"new").unwrap();
    let r = t::run(
        "mv",
        &["-v", "--backup", "-S", ".bak", format!("{}/hl", d).as_str(), format!("{}/old", d).as_str()],
        "",
    );
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read(format!("{}/old.bak", d)).unwrap(), b"new");
    let _ = fs::remove_dir_all(d);
}

#[test]
fn strip_slashes_and_operand_errors() {
    let d = "/tmp/mvt_ops";
    fx(d);
    // trailing slash on existing dir source: -v shows stripped path
    let r = t::run("mv", &["--strip-trailing-slashes", "-v", format!("{}/d1/", d).as_str(), format!("{}/d9", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let ev = r.err();
    assert!(ev.contains(&format!("{}/d1'", d)) && !ev.contains(&format!("{}/d1/'", d)), "{}", ev);
    assert!(fs::metadata(format!("{}/d9/sub/f3", d)).is_ok());
    let r = t::run("mv", &[], "");
    assert_ne!(r.code, 0);
    assert!(r.err().contains("missing file operand"), "{}", r.err());
    let r = t::run("mv", &[format!("{}/x_zz", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("missing destination file operand after"), "{}", r.err());
    let r = t::run("mv", &["--version"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("coreutils"));
    let r = t::run("mv", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Usage: mv"));
    let r = t::run("mv", &["-t", format!("{}/d9", d).as_str(), format!("{}/d9/sub/f3", d).as_str()], "");
    assert_eq!(r.code, 0);
    let r = t::run("mv", &["-t"], "");
    assert_eq!(r.code, 2);
    let _ = fs::remove_dir_all(d);
}

#[test]
fn fuzz_mv_roundtrip() {
    // invariant (man: mv renames): content after mv equals content before,
    // and the old name is gone; chain of renames keeps content addressable.
    let mut rng = t::Rng::new(20261021);
    let root = "/tmp/mvt_fuzz";
    for _ in 0..10 {
        let _ = fs::remove_dir_all(root);
        fs::create_dir_all(root).unwrap();
        let n = 3 + rng.below(5) as usize;
        let p = root.to_string();
        for _ in 0..n {
            let s: Vec<u8> = (0..8).map(|_| b'a' + (rng.below(26) as u8)).collect();
            let name = String::from_utf8(s).unwrap();
            let content: Vec<u8> = (0..(1 + rng.below(50) as u64)).map(|_| rng.below(256) as u8).collect();
            let src = format!("{}/src_{}", p, name);
            fs::write(&src, &content).unwrap();
            let dst = format!("{}/dst_{}", p, name);
            let r = t::run("mv", &[src.as_str(), dst.as_str()], "");
            assert_eq!(r.code, 0, "{}", r.err());
            assert!(!fs::metadata(&src).is_ok());
            assert_eq!(fs::read(&dst).unwrap(), content);
            // chain
            let dst2 = format!("{}/fin_{}", p, name);
            let r = t::run("mv", &[dst.as_str(), dst2.as_str()], "");
            assert_eq!(r.code, 0);
            assert_eq!(fs::read(&dst2).unwrap(), content);
        }
    }
}

#[test]
fn long_options_and_flags_matrix() {
    let d = "/tmp/mvt_long";
    fx(d);
    // -Z SELinux rejection
    let r = t::run("mv", &["-Z", format!("{}/f1", d).as_str(), format!("{}/g", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("SELinux"), "{}", r.err());
    // -S missing value
    let r = t::run("mv", &[format!("{}/f1", d).as_str(), format!("{}/g", d).as_str(), "-S"], "");
    assert_eq!(r.code, 2);
    // --target-directory= form
    fs::create_dir_all(format!("{}/tdir", d)).unwrap();
    let r = t::run("mv", &[format!("--target-directory={}/tdir", d).as_str(), format!("{}/d1/f2", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(fs::metadata(format!("{}/tdir/f2", d)).is_ok());
    // --backup=none disables backup
    fs::write(format!("{}/old", d), b"O").unwrap();
    fs::write(format!("{}/f1", d), b"N").unwrap();
    let r = t::run("mv", &["--backup=none", format!("{}/f1", d).as_str(), format!("{}/old", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert!(!fs::metadata(format!("{}/old~", d)).is_ok());
    // clustered -bfvn: -n wins last? no_clobber set → existing kept
    fs::write(format!("{}/f1", d), b"N2").unwrap();
    let r = t::run("mv", &["-bfvn", format!("{}/f1", d).as_str(), format!("{}/old", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/old", d)).unwrap(), b"N");
    // -u combined: update older silently skip (old newer)
    let r = t::run("mv", &["-unv", format!("{}/f1", d).as_str(), format!("{}/old", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/old", d)).unwrap(), b"N");
    // invalid clustered option
    let r = t::run("mv", &["-fQ", format!("{}/f1", d).as_str(), format!("{}/old", d).as_str()], "");
    assert_eq!(r.code, 2);
    // unrecognized long option
    let r = t::run("mv", &["--no-such-zz"], "");
    assert_eq!(r.code, 2);
    // -T with 3 operands → extra operand
    let r = t::run("mv", &["-T", "a_zz", "b_zz", "c_zz"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("extra operand"), "{}", r.err());
    // single operand, no dir → missing destination
    let r = t::run("mv", &[format!("{}/f1", d).as_str()], "");
    assert_eq!(r.code, 1);
    // --exchange
    fs::write(format!("{}/x1", d), b"A").unwrap();
    fs::write(format!("{}/x2", d), b"B").unwrap();
    let r = t::run("mv", &["--exchange", format!("{}/x1", d).as_str(), format!("{}/x2", d).as_str()], "");
    if r.code == 0 {
        assert_eq!(fs::read(format!("{}/x1", d)).unwrap(), b"B");
        assert_eq!(fs::read(format!("{}/x2", d)).unwrap(), b"A");
    }
    // -i Once (-I) prompt accept
    fs::write(format!("{}/f1", d), b"C").unwrap();
    let r = t::run("mv", &["-I", format!("{}/f1", d).as_str(), format!("{}/old", d).as_str()], "y\n");
    assert_eq!(r.code, 0);
    // mv file into dir given via trailing slash dest
    fs::write(format!("{}/f1", d), b"D").unwrap();
    let r = t::run("mv", &[format!("{}/f1", d).as_str(), format!("{}/tdir/", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/tdir/f1", d)).unwrap(), b"D");
    let _ = fs::remove_dir_all(d);
}

#[test]
fn cross_device_copy_fallback() {
    // /tmp (tmpfs) → /out (9p) is a different device: rename EXDEV → copy+unlink
    let src = "/tmp/mvt_xdev_src";
    let dst = "/out/mvt_xdev_dst";
    fs::write(src, b"XDATALOOP").unwrap();
    let r = t::run("mv", &["-v", src, dst], "");
    if r.code == 0 {
        assert_eq!(fs::read(dst).unwrap(), b"XDATALOOP");
        assert!(!fs::metadata(src).is_ok(), "source must be removed after fallback");
    } else {
        // --no-copy behavior for cross-device without fallback path
        assert!(r.err().contains("Invalid cross-device link") || r.err().contains("cross-device"), "{}", r.err());
    }
    let _ = fs::remove_file(src);
    let _ = fs::remove_file(dst);
    // --no-copy must not fallback
    fs::write(src, b"NC").unwrap();
    let r = t::run("mv", &["--no-copy", src, dst], "");
    assert_eq!(r.code, 1, "{}", r.out());
    assert!(r.err().contains("cross-device"), "{}", r.err());
    let _ = fs::remove_file(src);
    let _ = fs::remove_file(dst);
}

#[test]
fn clusters_space_forms_and_update_matrix() {
    let d = "/tmp/mvt_c2";
    fx(d);
    // cluster with attached -S value: "-bS.bk"
    fs::write(format!("{}/dstf", d), b"OLD").unwrap();
    fs::write(format!("{}/f1", d), b"NEW").unwrap();
    let r = t::run("mv", &["-bS.bk", format!("{}/f1", d).as_str(), format!("{}/dstf", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read(format!("{}/dstf.bk", d)).unwrap(), b"OLD");
    // -S in cluster consumes the next arg as suffix → then missing destination (rc1)
    let r = t::run("mv", &["-fS", format!("{}/f1", d).as_str()], "");
    assert_eq!(r.code, 1);
    // --backup=existing enables backup
    fs::write(format!("{}/f1", d), b"N1").unwrap();
    fs::write(format!("{}/dstf", d), b"OLD2").unwrap();
    let r = t::run("mv", &["--backup=existing", format!("{}/f1", d).as_str(), format!("{}/dstf", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(fs::metadata(format!("{}/dstf~", d)).is_ok());
    // clustered -I + accept
    fs::write(format!("{}/f1", d), b"N2").unwrap();
    let r = t::run("mv", &["-I", format!("{}/f1", d).as_str(), format!("{}/dstf", d).as_str()], "y\n");
    assert_eq!(r.code, 0, "{}", r.err());
    // -if: force overrides interactive (silent overwrite)
    fs::write(format!("{}/f1", d), b"N3").unwrap();
    let r = t::run("mv", &["-if", format!("{}/f1", d).as_str(), format!("{}/dstf", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read(format!("{}/dstf", d)).unwrap(), b"N3");
    // -T onto existing non-empty directory without -f → error
    fs::write(format!("{}/f1", d), b"N4").unwrap();
    let r = t::run("mv", &["-Tv", format!("{}/f1", d).as_str(), format!("{}/d1", d).as_str()], "");
    assert_eq!(r.code, 1, "{}", r.err());
    // -f -T onto empty dir: remove_dst(rmdir) then rename
    fs::create_dir(format!("{}/ed", d)).unwrap();
    let r = t::run("mv", &["-fT", format!("{}/f1", d).as_str(), format!("{}/ed", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read(format!("{}/ed", d)).unwrap(), b"N4");
    // same file via hardlink
    fs::write(format!("{}/hl1", d), b"H").unwrap();
    fs::hard_link(format!("{}/hl1", d), format!("{}/hl2", d)).unwrap();
    let r = t::run("mv", &[format!("{}/hl1", d).as_str(), format!("{}/hl2", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("same file"), "{}", r.err());
    // --strip-trailing-slashes
    fs::write(format!("{}/f1", d), b"S1").unwrap();
    let r = t::run("mv", &["--strip-trailing-slashes", format!("{}/f1", d).as_str(), format!("{}/sdst", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    // -u where source is NEWER: proceed with rename（dst mtime 設為過去避免時間戳同刻度）
    fs::write(format!("{}/newer", d), b"NEWER").unwrap();
    {
        let f = fs::OpenOptions::new().write(true).open(format!("{}/sdst", d)).unwrap();
        f.set_times(
            std::fs::FileTimes::new()
                .set_modified(std::time::SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100)),
        )
        .unwrap();
    }
    let r = t::run("mv", &["-u", format!("{}/newer", d).as_str(), format!("{}/sdst", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read(format!("{}/sdst", d)).unwrap(), b"NEWER");
    let _ = fs::remove_dir_all(d);
}
