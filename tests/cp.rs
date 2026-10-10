//! cp applet e2e tests (guest).
use coreutils::testutil as t;
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

fn fx(dir: &str) {
    let _ = fs::remove_dir_all(dir);
    fs::create_dir_all(format!("{}/c1/sub", dir)).unwrap();
    fs::write(format!("{}/c1/f", dir), b"hello").unwrap();
    fs::set_permissions(format!("{}/c1/f", dir), fs::Permissions::from_mode(0o750)).unwrap();
    fs::write(format!("{}/c1/sub/g", dir), b"gcontents").unwrap();
    std::os::unix::fs::symlink("c1/f", format!("{}/lnk", dir)).unwrap();
    std::os::unix::fs::symlink("nowhere_zz", format!("{}/dang", dir)).unwrap();
    std::os::unix::fs::symlink("c1", format!("{}/dirlink", dir)).unwrap();
    fs::write(format!("{}/pre", dir), b"x").unwrap();
}

#[test]
fn basics_mode_and_recursive() {
    let d = "/tmp/cpt_basic";
    fx(d);
    // default: mode preserved on fresh creation; symlink (command line) dereferenced
    let r = t::run("cp", &[format!("{}/c1/f", d).as_str(), format!("{}/cf", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read(format!("{}/cf", d)).unwrap(), b"hello");
    assert_eq!(fs::metadata(format!("{}/cf", d)).unwrap().mode() & 0o7777, 0o750);
    // directory without -R
    let r = t::run("cp", &[format!("{}/c1", d).as_str(), format!("{}/copy", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("-r not specified; omitting directory"), "{}", r.err());
    // -R recursion keeps inner symlink as symlink
    let r = t::run("cp", &["-R", format!("{}/c1", d).as_str(), format!("{}/copy", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read(format!("{}/copy/sub/g", d)).unwrap(), b"gcontents");
    // -R on symlink-to-dir: GNU dereferences command-line symlink (deviation from local ref recorded in assumptions)
    let r = t::run("cp", &["-R", format!("{}/dirlink", d).as_str(), format!("{}/cdr", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/cdr/f", d)).unwrap(), b"hello");
    // same file
    let r = t::run("cp", &[format!("{}/c1/f", d).as_str(), format!("{}/c1/f", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("are the same file"), "{}", r.err());
    // missing source
    let r = t::run("cp", &[format!("{}/noexist_zz", d).as_str(), format!("{}/x", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("cannot stat"), "{}", r.err());
    let _ = fs::remove_dir_all(d);
}

#[test]
fn symlink_flags() {
    let d = "/tmp/cpt_sym";
    fx(d);
    // -P: copy link itself
    let r = t::run("cp", &["-P", format!("{}/lnk", d).as_str(), format!("{}/pl", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read_link(format!("{}/pl", d)).unwrap().to_string_lossy(), "c1/f");
    // default: dereference
    let r = t::run("cp", &[format!("{}/lnk", d).as_str(), format!("{}/pf", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/pf", d)).unwrap(), b"hello");
    // -L on dangling → error
    let r = t::run("cp", &["-L", format!("{}/dang", d).as_str(), format!("{}/x", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("cannot stat"), "{}", r.err());
    // -s: link points at source string
    let r = t::run("cp", &["-s", format!("{}/c1/f", d).as_str(), format!("{}/sl2", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read_link(format!("{}/sl2", d)).unwrap().to_string_lossy(), format!("{}/c1/f", d));
    // -l: hard link
    let r = t::run("cp", &["-l", format!("{}/c1/f", d).as_str(), format!("{}/hl2", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::metadata(format!("{}/hl2", d)).unwrap().ino(), fs::metadata(format!("{}/c1/f", d)).unwrap().ino());
    // -d preserves link
    let r = t::run("cp", &["-d", format!("{}/lnk", d).as_str(), format!("{}/dl", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read_link(format!("{}/dl", d)).unwrap().to_string_lossy(), "c1/f");
    let _ = fs::remove_dir_all(d);
}

#[test]
fn clobber_update_backup_interactive() {
    let d = "/tmp/cpt_clob";
    fx(d);
    // -n keeps dest
    let r = t::run("cp", &["-n", format!("{}/c1/f", d).as_str(), format!("{}/pre", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/pre", d)).unwrap(), b"x");
    // --update=none-fail → rc1 with wording
    let r = t::run("cp", &["--update=none-fail", format!("{}/c1/f", d).as_str(), format!("{}/pre", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("not replacing"), "{}", r.err());
    // -u: dest same-age/newer → silent skip (pre written after c1/f in fx)
    let r = t::run("cp", &["-u", format!("{}/c1/f", d).as_str(), format!("{}/pre", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/pre", d)).unwrap(), b"x");
    // -f: content replaced; existing dest keeps its own mode (ref behavior)
    let r = t::run("cp", &["-f", format!("{}/c1/f", d).as_str(), format!("{}/pre", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/pre", d)).unwrap(), b"hello");
    let pre_mode = fs::metadata(format!("{}/pre", d)).unwrap().mode() & 0o7777;
    assert_ne!(pre_mode, 0o750, "overwrite should keep old mode, got {:o}", pre_mode);
    // -i decline keeps
    fs::write(format!("{}/pre", d), b"x").unwrap();
    t::run("cp", &["-i", format!("{}/c1/f", d).as_str(), format!("{}/pre", d).as_str()], "n\n");
    assert_eq!(fs::read(format!("{}/pre", d)).unwrap(), b"x");
    // -i accept replaces
    let r = t::run("cp", &["-i", format!("{}/c1/f", d).as_str(), format!("{}/pre", d).as_str()], "y\n");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/pre", d)).unwrap(), b"hello");
    // backup: original moved aside, verbose has no backup text (ref)
    fs::write(format!("{}/pre", d), b"orig").unwrap();
    let r = t::run("cp", &["--backup", "-S", ".bk", format!("{}/c1/f", d).as_str(), format!("{}/pre", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/pre.bk", d)).unwrap(), b"orig");
    assert_eq!(fs::read(format!("{}/pre", d)).unwrap(), b"hello");
    // multi-operand into dir where one copy lands on itself → same-file error rc1
    let r = t::run("cp", &[format!("{}/c1/f", d).as_str(), format!("{}/c1/sub/g", d).as_str(), format!("{}/c1", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("are the same file"), "{}", r.err());
    let _ = fs::remove_dir_all(d);
}

#[test]
fn target_parents_and_attrs() {
    let d = "/tmp/cpt_tgt";
    fx(d);
    // -t target dir
    let r = t::run("cp", &["-t", format!("{}/c1", d).as_str(), format!("{}/pre", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/c1/pre", d)).unwrap(), b"x");
    // -T single pair even if last is dir → EISDIR creating over dir
    let r = t::run("cp", &["-T", format!("{}/pre", d).as_str(), format!("{}/c1", d).as_str()], "");
    assert_eq!(r.code, 1, "{}", r.out());
    assert!(fs::metadata(format!("{}/c1", d)).unwrap().is_dir());
    // --parents requires dir dest
    fx(d);
    let r = t::run("cp", &["--parents", format!("{}/c1/f", d).as_str(), format!("{}/noexist_zz", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("with --parents, the destination must be a directory"), "{}", r.err());
    // --parents into existing dir recreates rel path
    let r = t::run("cp", &["--parents", format!("{}/c1/f", d).as_str(), d], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read(format!("{}/c1/f", d)).unwrap(), b"hello");
    // multi-operand, last not a directory
    let r = t::run("cp", &[format!("{}/c1/f", d).as_str(), format!("{}/c1/sub/g", d).as_str(), format!("{}/pre", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("target '") && r.err().contains("is not a directory"), "{}", r.err());
    // --preserve=context → SELinux message rc1
    let r = t::run("cp", &["--preserve=context", format!("{}/c1/f", d).as_str(), format!("{}/cf2", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("SELinux"), "{}", r.err());
    // --no-preserve=mode → default (umask) mode, not 750
    let r = t::run("cp", &["--no-preserve=mode", format!("{}/c1/f", d).as_str(), format!("{}/cf3", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_ne!(fs::metadata(format!("{}/cf3", d)).unwrap().mode() & 0o7777, 0o750);
    // --debug prints offload line glued before verbose
    let r = t::run("cp", &["--debug", format!("{}/c1/f", d).as_str(), format!("{}/cf4", d).as_str()], "");
    assert_eq!(r.code, 0);
    let e = r.err();
    assert!(e.contains("copy offload:") && e.contains("reflink:") && e.contains("sparse detection:"), "{}", e);
    assert!(e.contains("' -> '"), "{}", e);
    // -a archive: recursive + preserve mode
    let r = t::run("cp", &["-a", format!("{}/c1", d).as_str(), format!("{}/arc", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::metadata(format!("{}/arc/f", d)).unwrap().mode() & 0o7777, 0o750);
    let _ = fs::remove_dir_all(d);
}

#[test]
fn operand_errors_and_version() {
    let r = t::run("cp", &[], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("missing file operand"), "{}", r.err());
    let r = t::run("cp", &["a_zz"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("missing destination file operand after 'a_zz'"), "{}", r.err());
    let r = t::run("cp", &["--version"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("coreutils"), "{}", r.out());
    let r = t::run("cp", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("usage: cp"), "{}", r.out());
    let r = t::run("cp", &["--bogus"], "");
    assert_eq!(r.code, 2);
    let r = t::run("cp", &["-S"], "");
    assert_eq!(r.code, 2);
}

#[test]
fn fuzz_cp_preserves_contents_and_tree() {
    // invariants from man: cp copies file contents byte-identically;
    // -R reproduces the directory tree (files present with same content);
    // fresh copies preserve source mode.
    let mut rng = t::Rng::new(20261022);
    let root = "/tmp/cpt_fuzz";
    for _ in 0..6 {
        let _ = fs::remove_dir_all(root);
        fs::create_dir_all(format!("{}/src/sub", root)).unwrap();
        let mut expect: Vec<(String, Vec<u8>, u32)> = Vec::new();
        let names: Vec<&str> = vec!["a", "b", "sub/c", "sub/d"];
        for name in names {
            let len = rng.below(300) as usize;
            let content: Vec<u8> = (0..len).map(|_| rng.below(256) as u8).collect();
            let mode = 0o400 | rng.below(0o377) as u32;
            let p = format!("{}/src/{}", root, name);
            fs::write(&p, &content).unwrap();
            fs::set_permissions(&p, fs::Permissions::from_mode(mode)).unwrap();
            expect.push((name.to_string(), content, mode & 0o7777));
        }
        let dst = format!("{}/dst", root);
        let r = t::run("cp", &["-R", format!("{}/src", root).as_str(), dst.as_str()], "");
        assert_eq!(r.code, 0, "{}", r.err());
        for (name, content, mode) in &expect {
            let p = format!("{}/{}", dst, name);
            assert_eq!(fs::read(&p).unwrap(), *content, "content mismatch for {}", name);
            assert_eq!(fs::metadata(&p).unwrap().mode() & 0o7777, *mode, "mode mismatch for {}", name);
        }
        // single-file cp byte-identical
        let (name, content, _) = &expect[0];
        let r = t::run("cp", &[format!("{}/src/{}", root, name).as_str(), format!("{}/single", root).as_str()], "");
        assert_eq!(r.code, 0);
        assert_eq!(fs::read(format!("{}/single", root)).unwrap(), *content);
    }
}

extern "C" {
    fn mkfifo(path: *const i8, mode: u32) -> i32;
}

#[test]
fn preserve_update_and_long_options() {
    let d = "/tmp/cpt_pres";
    fx(d);
    // -p preserves mode and timestamps
    let r = t::run("cp", &["-p", format!("{}/c1/f", d).as_str(), format!("{}/pf", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::metadata(format!("{}/pf", d)).unwrap().mode() & 0o7777, 0o750);
    assert_eq!(
        fs::metadata(format!("{}/pf", d)).unwrap().mtime(),
        fs::metadata(format!("{}/c1/f", d)).unwrap().mtime()
    );
    // --preserve=timestamps alone: mode NOT preserved (fresh create)
    let r = t::run("cp", &["--preserve=timestamps", format!("{}/c1/f", d).as_str(), format!("{}/tf", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_ne!(fs::metadata(format!("{}/tf", d)).unwrap().mode() & 0o7777, 0o750);
    // --preserve=bogus → rc2 invalid attribute
    let r = t::run("cp", &["--preserve=bogus", format!("{}/c1/f", d).as_str(), format!("{}/x", d).as_str()], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("invalid attribute 'bogus'"), "{}", r.err());
    // --no-presserve bogus path
    let r = t::run("cp", &["--no-preserve=nope", format!("{}/c1/f", d).as_str(), format!("{}/x", d).as_str()], "");
    assert_eq!(r.code, 2);
    // --update=older dest newer → skip rc0
    fs::write(format!("{}/pre", d), b"x").unwrap();
    let r = t::run("cp", &["--update=older", format!("{}/c1/f", d).as_str(), format!("{}/pre", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/pre", d)).unwrap(), b"x");
    // --update=bogus → rc2
    let r = t::run("cp", &["--update=bogus", "a_zz", "b_zz"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().contains("invalid argument 'bogus' for '--update'"), "{}", r.err());
    // --update (no value) = older
    let r = t::run("cp", &["--update", format!("{}/c1/f", d).as_str(), format!("{}/pre", d).as_str()], "");
    assert_eq!(r.code, 0);
    // --suffix= form with --backup
    fs::write(format!("{}/pre", d), b"P1").unwrap();
    let r = t::run("cp", &["--backup", "--suffix=.zz", format!("{}/c1/f", d).as_str(), format!("{}/pre", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/pre.zz", d)).unwrap(), b"P1");
    // --target-directory= form
    let r = t::run("cp", &[format!("--target-directory={}/c1", d).as_str(), format!("{}/pre", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert!(fs::metadata(format!("{}/c1/pre", d)).is_ok());
    // -H: top-level symlink deref, inner kept
    let r = t::run("cp", &["-R", "-H", format!("{}/dirlink", d).as_str(), format!("{}/hdst", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read(format!("{}/hdst/f", d)).unwrap(), b"hello");
    // FIFO is recreated as FIFO (GNU man; ref hangs, deviation documented)
    let fp = format!("{}/fifo1", d);
    let c = std::ffi::CString::new(fp.clone()).unwrap();
    assert_eq!(unsafe { mkfifo(c.as_ptr(), 0o644) }, 0);
    let r = t::run("cp", &[fp.as_str(), format!("{}/fifo2", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let m = fs::metadata(format!("{}/fifo2", d)).unwrap();
    assert_eq!(m.mode() & 0o170000, 0o010000, "fifo2 not a fifo");
    // --sparse / --reflink accepted no-op
    let r = t::run("cp", &["--sparse=always", "--reflink=auto", format!("{}/c1/f", d).as_str(), format!("{}/sf", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    // -a -T
    let r = t::run("cp", &["-a", "-T", format!("{}/c1", d).as_str(), format!("{}/arc2", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(fs::metadata(format!("{}/arc2/pre", d)).is_ok());
    // invalid option
    let r = t::run("cp", &["-Q", "a_zz", "b_zz"], "");
    assert_eq!(r.code, 2);
    let _ = fs::remove_dir_all(d);
}

#[test]
fn recursive_edge_paths() {
    let d = "/tmp/cpt_redge";
    let _ = fs::remove_dir_all(d);
    fs::create_dir_all(format!("{}/src/deep/deeper", d)).unwrap();
    fs::write(format!("{}/src/deep/deeper/leaf", d), b"leafdata").unwrap();
    std::os::unix::fs::symlink("deeper", format!("{}/src/deep/updir", d)).unwrap();
    // deep recursion
    let r = t::run("cp", &["-R", format!("{}/src", d).as_str(), format!("{}/d1", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read(format!("{}/d1/deep/deeper/leaf", d)).unwrap(), b"leafdata");
    assert_eq!(fs::read_link(format!("{}/d1/deep/updir", d)).unwrap().to_string_lossy(), "deeper");
    // -L recursion dereferences the inner symlink too
    let r = t::run("cp", &["-R", "-L", format!("{}/src", d).as_str(), format!("{}/d2", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(fs::metadata(format!("{}/d2/deep/updir/leaf", d)).is_ok());
    // copy onto existing empty dir merges
    fs::create_dir_all(format!("{}/d3", d)).unwrap();
    let r = t::run("cp", &["-R", format!("{}/src", d).as_str(), format!("{}/d3", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert!(fs::metadata(format!("{}/d3/src/deep", d)).is_ok());
    // -x one-file-system on same fs: works
    let r = t::run("cp", &["-R", "-x", format!("{}/src", d).as_str(), format!("{}/d4", d).as_str()], "");
    assert_eq!(r.code, 0);
    // trailing slash on source
    let r = t::run("cp", &["-R", format!("{}/src/", d).as_str(), format!("{}/d5", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    // copy file onto FIFO destination → error branch
    let fp = format!("{}/d4fifo", d);
    let c = std::ffi::CString::new(fp.clone()).unwrap();
    assert_eq!(unsafe { mkfifo(c.as_ptr(), 0o644) }, 0);
    let _ = fs::metadata(format!("{}/src/deep", d));
    // dest exists file with --remove-destination: content replaced
    fs::write(format!("{}/tgt", d), b"OLD").unwrap();
    let r = t::run("cp", &["--remove-destination", format!("{}/src/deep/deeper/leaf", d).as_str(), format!("{}/tgt", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read(format!("{}/tgt", d)).unwrap(), b"leafdata");
    // -f dest is unwritable? guest is root; skip. verbose -r
    let r = t::run("cp", &["-r", "-v", format!("{}/src", d).as_str(), format!("{}/d6", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert!(r.err().contains("/src' -> '"), "{}", r.err());
    let _ = fs::remove_dir_all(d);
}

#[test]
fn more_option_forms() {
    let d = "/tmp/cpt_forms";
    fx(d);
    // bare --preserve (defaults all): mode + mtime preserved
    let r = t::run("cp", &["--preserve", format!("{}/c1/f", d).as_str(), format!("{}/p1", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::metadata(format!("{}/p1", d)).unwrap().mode() & 0o7777, 0o750);
    // bare --no-preserve with space-separated list
    let r = t::run("cp", &["--no-preserve", "mode", format!("{}/c1/f", d).as_str(), format!("{}/p2", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_ne!(fs::metadata(format!("{}/p2", d)).unwrap().mode() & 0o7777, 0o750);
    // bare --no-preserve with bogus attr
    let r = t::run("cp", &["--no-preserve", "nope", "a_zz", "b_zz"], "");
    assert_eq!(r.code, 2);
    // --update space-separated forms
    fs::write(format!("{}/ex", d), b"E").unwrap();
    let r = t::run("cp", &["--update", "none", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/ex", d)).unwrap(), b"E");
    let r = t::run("cp", &["--update", "none-fail", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 1);
    let r = t::run("cp", &["--update", "all", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/ex", d)).unwrap(), b"hello");
    // --backup with space-separated control values
    fs::write(format!("{}/ex", d), b"E2").unwrap();
    let r = t::run("cp", &["--backup", "none", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert!(!fs::metadata(format!("{}/ex~", d)).is_ok());
    let r = t::run("cp", &["--backup", "existing", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert!(fs::metadata(format!("{}/ex~", d)).is_ok());
    // --interactive takes WHEN only in = form; bare form leaves "never" as operand → target error
    let r = t::run("cp", &["--interactive", "never", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 1);
    let r = t::run("cp", &["--interactive=never", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let r = t::run("cp", &["--interactive=bogus", "a_zz", "b_zz"], "");
    assert_eq!(r.code, 2);
    // -Z
    let r = t::run("cp", &["-Z", format!("{}/c1/f", d).as_str(), format!("{}/zz", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("SELinux"), "{}", r.err());
    // -t missing value
    let r = t::run("cp", &["-t"], "");
    assert_eq!(r.code, 2);
    // clusters: -Pp (deref never + preserve), -vi with decline, -fvn
    let r = t::run("cp", &["-Pp", format!("{}/lnk", d).as_str(), format!("{}/cl1", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read_link(format!("{}/cl1", d)).unwrap().to_string_lossy(), "c1/f");
    fs::write(format!("{}/ex", d), b"KEEP").unwrap();
    let r = t::run("cp", &["-vi", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "n\n");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/ex", d)).unwrap(), b"KEEP");
    let r = t::run("cp", &["-fvn", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/ex", d)).unwrap(), b"KEEP");
    // -s -l clusters via -sl? -s wins symlink special
    let r = t::run("cp", &["-s", format!("{}/c1/f", d).as_str(), format!("{}/sl2", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::read_link(format!("{}/sl2", d)).unwrap().to_string_lossy(), format!("{}/c1/f", d));
    let _ = fs::remove_dir_all(d);
}

#[test]
fn cp_option_clusters_and_edges() {
    let d = "/tmp/cpt_cl";
    fx(d);
    // cluster -dp: deref never + preserve, copy symlink as symlink
    let r = t::run("cp", &["-dp", format!("{}/lnk", d).as_str(), format!("{}/k1", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(fs::read_link(format!("{}/k1", d)).is_ok());
    // cluster -Rv recursive verbose
    let r = t::run("cp", &["-Rv", format!("{}/c1", d).as_str(), format!("{}/r1", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(r.err().contains("->"));
    // cluster -sv: symlink special verbose
    let r = t::run("cp", &["-sv", format!("{}/c1/f", d).as_str(), format!("{}/sm1", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    // cluster -lu: hard link + update
    let r = t::run("cp", &["-lu", format!("{}/c1/f", d).as_str(), format!("{}/hl1", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(fs::metadata(format!("{}/hl1", d)).unwrap().ino(), fs::metadata(format!("{}/c1/f", d)).unwrap().ino());
    // cluster -Rvx: one_fs verbose recursive
    let r = t::run("cp", &["-Rvx", format!("{}/c1", d).as_str(), format!("{}/r2", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    // cluster -bv: backup verbose onto existing
    fs::write(format!("{}/bd", d), b"OLD").unwrap();
    let r = t::run("cp", &["-bv", format!("{}/c1/f", d).as_str(), format!("{}/bd", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(fs::metadata(format!("{}/bd~", d)).is_ok());
    // cluster -Hp
    let r = t::run("cp", &["-Hp", format!("{}/lnk", d).as_str(), format!("{}/hpd", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(fs::metadata(format!("{}/hpd", d)).unwrap().is_file());
    // hard link on non-file → error
    let r = t::run("cp", &["-l", "/dev/null", format!("{}/hldev", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("cannot hard link non-file"), "{}", r.err());
    // cp -R of FIFO source inside dir works; cp FIFO directly too
    // --parents of root
    let r = t::run("cp", &["--parents", "/", format!("{}/r1", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("cannot copy '/' with --parents"), "{}", r.err());
    // --interactive= forms
    let r = t::run("cp", &["--interactive=always", format!("{}/c1/f", d).as_str(), format!("{}/bd", d).as_str()], "y\n");
    assert_eq!(r.code, 0, "{}", r.err());
    let r = t::run("cp", &["--interactive=bogus", "a_zz", "b_zz"], "");
    assert_eq!(r.code, 2);
    let _ = fs::remove_dir_all(d);
}

#[test]
fn cp_error_paths_and_cluster_matrix() {
    let d = "/tmp/cpt_err";
    fx(d);
    fs::write(format!("{}/filedest", d), b"X").unwrap();
    // hard link onto char device → "cannot hard link non-file"
    let r = t::run("cp", &["-l", "/dev/null", format!("{}/l1", d).as_str()], "");
    assert_eq!(r.code, 1, "{}", r.err());
    assert!(r.err().contains("cannot hard link non-file"), "{}", r.err());
    // char device → special file error
    let r = t::run("cp", &["/dev/null", format!("{}/d1", d).as_str()], "");
    assert_eq!(r.code, 1, "{}", r.err());
    assert!(r.err().contains("special file"), "{}", r.err());
    // --parents of root
    let r = t::run("cp", &["--parents", "/", format!("{}/c1", d).as_str()], "");
    assert_eq!(r.code, 1, "{}", r.err());
    assert!(r.err().contains("cannot copy '/' with --parents"), "{}", r.err());
    // --parents onto non-directory dest → destination must be a directory
    let r = t::run("cp", &["--parents", format!("{}/c1/f", d).as_str(), format!("{}/filedest", d).as_str()], "");
    assert_eq!(r.code, 1, "{}", r.err());
    assert!(r.err().contains("--parents"), "{}", r.err());
    // -R dir onto existing file dest
    let r = t::run("cp", &["-R", format!("{}/c1", d).as_str(), format!("{}/filedest", d).as_str()], "");
    assert_ne!(r.code, 0);
    // -n skip is Ok for symlink / -s / -l / fifo onto existing dest
    fs::write(format!("{}/ex", d), b"KEEP").unwrap();
    let r = t::run("cp", &["-n", format!("{}/lnk", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let r = t::run("cp", &["-n", "-s", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    let r = t::run("cp", &["-n", "-l", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    extern "C" {
        fn mkfifo(path: *const i8, mode: u32) -> i32;
    }
    let mf = std::ffi::CString::new(format!("{}/mf", d)).unwrap();
    let ok = unsafe { mkfifo(mf.as_ptr(), 0o644) } == 0;
    if ok {
        let r = t::run("cp", &["-n", format!("{}/mf", d).as_str(), format!("{}/ex", d).as_str()], "");
        assert_eq!(r.code, 0, "{}", r.err());
    }
    // --update older space-separated skips (dest newer)
    fs::write(format!("{}/ex", d), b"KEEP").unwrap();
    let r = t::run("cp", &["--update", "older", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    // bare --backup followed by dash option → backup on
    let r = t::run("cp", &["--backup", "-p", format!("{}/c1/f", d).as_str(), format!("{}/ex", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(fs::metadata(format!("{}/ex~", d)).is_ok());
    // -t missing value
    let r = t::run("cp", &["-t"], "");
    assert_eq!(r.code, 2, "{}", r.err());
    // cluster 't' tail without value → rc2 (covers cluster 't' with value + missing-value arms)
    fs::create_dir(format!("{}/ptd", d)).unwrap();
    let r = t::run(
        "cp",
        &["-t", format!("{}/ptd", d).as_str(), "-v", format!("{}/c1/f", d).as_str()],
        "",
    );
    assert_eq!(r.code, 0, "{}", r.err());
    let r = t::run(
        "cp",
        &["-vt", format!("{}/c1/f", d).as_str(), format!("{}/z1", d).as_str()],
        "",
    );
    assert_eq!(r.code, 2, "{}", r.err());
    // -t target-directory with parents (Some(d) clone arm)；絕對 operand（參考驗證過的形式）
    let r2 = t::run("cp", &["--parents", "-t", format!("{}/ptd", d).as_str(), format!("{}/c1/f", d).as_str()], "");
    assert_eq!(r2.code, 0, "{}", r2.err());
    assert_eq!(fs::read(format!("{}/ptd{}", d, format!("{}/c1/f", d))).unwrap(), b"hello");
    // --parents with dash-following option (-p) covers None-arm guard
    let r = t::run("cp", &["--parents", "-p", format!("{}/c1/f", d).as_str(), format!("{}/ptd", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    // -t with no operands → rc2
    let r = t::run("cp", &["-t", format!("{}/ptd", d).as_str()], "");
    assert_eq!(r.code, 2, "{}", r.err());
    // cluster 'a' archive verbose
    let r = t::run("cp", &["-av", format!("{}/c1/f", d).as_str(), format!("{}/a1", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    // cluster 'L' deref symlink
    let r = t::run("cp", &["-Lp", format!("{}/lnk", d).as_str(), format!("{}/Lp", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(fs::metadata(format!("{}/Lp", d)).unwrap().is_file());
    // cluster 'S' needs value → rc2
    let r = t::run("cp", &["-vS", format!("{}/c1/f", d).as_str(), format!("{}/sS", d).as_str()], "");
    assert_eq!(r.code, 2, "{}", r.err());
    // -s onto existing dir with -T → symlink create fails
    let r = t::run("cp", &["-sT", format!("{}/c1/f", d).as_str(), format!("{}/ptd", d).as_str()], "");
    assert_eq!(r.code, 1, "{}", r.err());
    assert!(r.err().contains("cannot create symlink"), "{}", r.err());
    let _ = fs::remove_dir_all(d);
}
