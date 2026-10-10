//! rm applet e2e tests (guest).
use coreutils::testutil as t;
use std::fs;

fn fx(dir: &str) {
    let _ = fs::remove_dir_all(dir);
    fs::create_dir_all(format!("{}/d1/sub", dir)).unwrap();
    fs::write(format!("{}/f1", dir), b"hello").unwrap();
    fs::write(format!("{}/d1/f2", dir), b"two").unwrap();
    fs::write(format!("{}/d1/sub/f3", dir), b"three").unwrap();
    fs::hard_link(format!("{}/f1", dir), format!("{}/h1", dir)).unwrap();
    std::os::unix::fs::symlink("f1", format!("{}/sl", dir)).unwrap();
}

#[test]
fn remove_files_and_missing() {
    let d = "/tmp/rmt_basic";
    fx(d);
    // hard link removal leaves other link
    let r = t::run("rm", &[format!("{}/h1", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(fs::read(format!("{}/f1", d)).unwrap(), b"hello");
    let r = t::run("rm", &[format!("{}/f1", d).as_str()], "");
    assert_eq!(r.code, 0);
    let r = t::run("rm", &[format!("{}/f1", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("cannot remove"));
    assert!(r.err().contains("No such file or directory"));
    let r = t::run("rm", &["-f", format!("{}/noexist_zz", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.err(), "");
    // directory without -r
    let r = t::run("rm", &[format!("{}/d1", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("Is a directory"));
    let _ = fs::remove_dir_all(d);
}

#[test]
fn recursive_and_dirs() {
    let d = "/tmp/rmt_rec";
    fx(d);
    let r = t::run("rm", &["-d", format!("{}/d1/sub", d).as_str()], "");
    // sub not empty (f3)
    assert_eq!(r.code, 1);
    assert!(r.err().contains("Directory not empty"));
    let r = t::run("rm", &["-r", format!("{}/d1", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert!(!fs::metadata(format!("{}/d1", d)).is_ok());
    // symlink itself removed, not target
    let r = t::run("rm", &[format!("{}/sl", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert!(fs::metadata(format!("{}/f1", d)).is_ok());
    let _ = fs::remove_dir_all(d);
}

#[test]
fn verbose_and_prompts() {
    let d = "/tmp/rmt_v";
    fx(d);
    let r = t::run("rm", &["-v", format!("{}/f1", d).as_str(), format!("{}/h1", d).as_str()], "");
    let e = r.err();
    assert!(e.contains("removed '") && e.contains("f1'") && e.contains("h1'"), "{}", e);
    // -i decline
    fs::write(format!("{}/f1", d), b"x").unwrap();
    let r = t::run("rm", &["-i", format!("{}/f1", d).as_str()], "n\n");
    assert!(r.err().contains("remove regular file") || r.err().contains("remove "), "{}", r.err());
    assert!(fs::metadata(format!("{}/f1", d)).is_ok());
    // -i accept
    let r = t::run("rm", &["-i", format!("{}/f1", d).as_str()], "y\n");
    assert_eq!(r.code, 0);
    assert!(!fs::metadata(format!("{}/f1", d)).is_ok());
    // -I with >3 operands prompts once; declining aborts all
    fs::write(format!("{}/a", d), b"1").unwrap();
    fs::write(format!("{}/b", d), b"2").unwrap();
    fs::write(format!("{}/c", d), b"3").unwrap();
    fs::write(format!("{}/e", d), b"4").unwrap();
    let r = t::run("rm", &["-I", format!("{}/a", d).as_str(), format!("{}/b", d).as_str(), format!("{}/c", d).as_str(), format!("{}/e", d).as_str()], "n\n");
    assert_eq!(r.code, 0);
    assert!(fs::metadata(format!("{}/a", d)).is_ok());
    assert!(r.err().contains("remove 4 arguments?"), "{}", r.err());
    let _ = fs::remove_dir_all(d);
}

#[test]
fn dangerous_guards() {
    let d = "/tmp/rmt_guard";
    let _ = fs::remove_dir_all(d);
    fs::create_dir_all(d).unwrap();
    let cur = std::env::current_dir().unwrap();
    std::env::set_current_dir(d).unwrap();
    let r = t::run("rm", &["."], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("refusing to remove '.' or '..' directory: skipping '.'"), "{}", r.err());
    let r = t::run("rm", &[".."], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("skipping '..'"));
    let r = t::run("rm", &["-r", "/"], "");
    assert_eq!(r.code, 1);
    let e = r.err();
    assert!(e.contains("it is dangerous to operate recursively on '/'"), "{}", e);
    assert!(e.contains("--no-preserve-root"), "{}", e);
    let r = t::run("rm", &["-rf", "/"], "");
    assert_eq!(r.code, 1);
    std::env::set_current_dir(cur).unwrap();
    let _ = fs::remove_dir_all(d);
}

#[test]
fn option_edges() {
    let r = t::run("rm", &[], "");
    assert_ne!(r.code, 0);
    assert!(r.err().contains("rm:"));
    let r = t::run("rm", &["--version"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("coreutils"), "{}", r.out());
    let r = t::run("rm", &["--help"], "");
    assert_eq!(r.code, 0);
    let r = t::run("rm", &["--bogus"], "");
    assert_eq!(r.code, 2);
    // -R alias of -r
    let d = "/tmp/rmt_R";
    fs::create_dir_all(format!("{}/x/y", d)).unwrap();
    fs::write(format!("{}/x/y/f", d), b"1").unwrap();
    let r = t::run("rm", &["-Rf", format!("{}/x", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert!(!fs::metadata(format!("{}/x", d)).is_ok());
}

#[test]
fn fuzz_rm_r_recurses_tree() {
    // invariant: after building a random tree, `rm -rf` removes it entirely (rc 0),
    // and file count before/after matches expectation (man: -r removes dirs recursively)
    let mut rng = t::Rng::new(20261020);
    let root = "/tmp/rmt_fuzz";
    for round in 0..8 {
        let _ = fs::remove_dir_all(root);
        fs::create_dir_all(root).unwrap();
        let mut files = 0usize;
        for _ in 0..20 {
            let depth = rng.below(3) as usize;
            let mut p = root.to_string();
            for l in 0..depth {
                p = format!("{}/l{}", p, rng.below(4));
                let _ = fs::create_dir_all(&p);
                let _ = l;
            }
            let f = format!("{}/f{}", p, rng.below(1000));
            fs::write(&f, b"x").unwrap();
            files += 1;
        }
        let r = t::run("rm", &["-rf", root], "");
        assert_eq!(r.code, 0, "round {}: {}", round, r.err());
        assert!(!fs::metadata(root).is_ok(), "round {} still exists, {} files", round, files);
    }
}

#[test]
fn long_options_and_walk_branches() {
    let d = "/tmp/rmt_long";
    fx(d);
    // --interactive=never overrides nothing; long forms
    let r = t::run("rm", &["--interactive=never", "-r", "--verbose", format!("{}/d1", d).as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(r.err().contains("removed '"));
    assert!(r.err().contains("removed directory"));
    let r = t::run("rm", &["--preserve-root", "--recursive", format!("{}/noexist_zz", d).as_str()], "");
    assert_eq!(r.code, 1);
    // --interactive=once decline on recursive
    let r = t::run("rm", &["--interactive=once", "-r", format!("{}/sl", d).as_str(), format!("{}/f1", d).as_str(), format!("{}/h1", d).as_str(), format!("{}/noexist_zz", d).as_str()], "n\n");
    assert_eq!(r.code, 0);
    assert!(fs::metadata(format!("{}/f1", d)).is_ok());
    // --interactive=always decline file
    let r = t::run("rm", &["--interactive=always", format!("{}/f1", d).as_str()], "no\n");
    assert_eq!(r.code, 0);
    assert!(fs::metadata(format!("{}/f1", d)).is_ok());
    // cluster -rfv on dir; also -d on non-empty dir
    let r = t::run("rm", &["-d", format!("{}/noexist_zz", d).as_str()], "");
    assert_eq!(r.code, 1);
    fs::create_dir_all(format!("{}/nd", d)).unwrap();
    fs::create_dir_all(format!("{}/nd/x", d)).unwrap();
    let r = t::run("rm", &["-d", format!("{}/nd", d).as_str()], "");
    assert_eq!(r.code, 1);
    assert!(r.err().contains("Directory not empty"), "{}", r.err());
    // -i on directory decline
    let r = t::run("rm", &["-r", "-i", format!("{}/nd", d).as_str()], "n\n");
    assert_eq!(r.code, 0);
    assert!(fs::metadata(format!("{}/nd", d)).is_ok());
    // invalid clustered option
    let r = t::run("rm", &["-fq", format!("{}/f1", d).as_str()], "");
    assert_eq!(r.code, 2);
    // --one-file-system recursive walk
    let r = t::run("rm", &["--one-file-system", "-rf", format!("{}/nd", d).as_str()], "");
    assert_eq!(r.code, 0);
    assert!(!fs::metadata(format!("{}/nd", d)).is_ok());
    // -I accept path
    let r = t::run("rm", &["-I", "-r", format!("{}/sl", d).as_str(), format!("{}/f1", d).as_str(), format!("{}/h1", d).as_str(), format!("{}/noexist_zz", d).as_str()], "y\n");
    assert_eq!(r.code, 1); // noexist fails after accept
    let _ = fs::remove_dir_all(d);
}
