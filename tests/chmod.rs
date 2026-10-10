//! chmod：octal/symbolic 模式、-c/-v/-f/-R/-h/--reference。
use coreutils::testutil as t;
use std::os::unix::fs::PermissionsExt;

fn fresh(dir: &str, name: &str, mode: u32) -> String {
    let p = format!("{}/{}", dir, name);
    std::fs::write(&p, b"x").unwrap();
    std::fs::set_permissions(&p, std::fs::Permissions::from_mode(mode)).unwrap();
    p
}

fn mode_of(p: &str) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::symlink_metadata(p).map(|m| m.permissions().mode() & 0o7777)
        .or_else(|_| std::fs::metadata(p).map(|m| m.permissions().mode() & 0o7777))
        .unwrap()
}

#[test]
fn octal_absolute() {
    let d = "/tmp/chmod_t1";
    let _ = std::fs::remove_dir_all(d);
    std::fs::create_dir_all(d).unwrap();
    for spec in ["644", "777", "0777", "7777", "04755", "0", "000"] {
        let p = fresh(d, format!("f_{}", spec).as_str(), 0o600);
        let r = t::run("chmod", &[spec, &p], "");
        assert_eq!(r.code, 0, "{}", r.err());
        let expect = u32::from_str_radix(spec, 8).unwrap();
        assert_eq!(mode_of(&p), expect, "spec {}", spec);
    }
}

#[test]
fn symbolic_matrix() {
    let d = "/tmp/chmod_t2";
    let _ = std::fs::remove_dir_all(d);
    std::fs::create_dir_all(d).unwrap();
    let cases: Vec<(&str, u32)> = vec![
        ("a+x", 0o711),
        ("u+x", 0o700),
        ("a+r", 0o644),
        ("go+w", 0o622),
        ("a+s", 0o6600),
        ("u+s", 0o4600),
        ("o+t", 0o1600),
        ("a-s", 0o600),
        ("u=rwxs", 0o4700),
        ("g=s", 0o2600),
        ("a=r", 0o444),
        ("a+x,go-w", 0o711),
        ("u=rwx,go=rx", 0o755),
        ("go-w", 0o600),
        ("a=rwx+s", 0o6777),
        ("u=r", 0o400),
        ("o=x", 0o601),
        ("ug=rwx,o=rx", 0o775),
        ("ug+s", 0o6600),
        ("a-s", 0o600),
    ];
    // 參考 uutils 拒絕無 who/op 的裸 "t"（GNU 接受）：措辭追參考
    let r0 = {
        let p = fresh(d, "bare_t", 0o600);
        let r = t::run("chmod", &["t", &p], "");
        assert_eq!(r.code, 1);
        assert_eq!(
            r.err().trim_end(),
            "chmod: invalid operator (expected +, -, or =, but found t)"
        );
        r
    };
    let _ = r0;
    {
        // 省略 who 的 "+w"：bits = perms & ~umask（umask 即時探測）
        let probe = format!("{}/umask_probe", d);
        let _ = std::fs::remove_dir_all(&probe);
        std::fs::create_dir(&probe).unwrap();
        let um = 0o777 & !mode_of(&probe);
        let p = fresh(d, &format!("um_{:o}", um), 0o600);
        let r = t::run("chmod", &["a=r,+w", &p], "");
        assert_eq!(r.code, 0, "{}", r.err());
        assert_eq!(mode_of(&p), 0o444 | (0o222 & !um), "umask={:o}", um);
    }
    for (spec, want) in cases {
        let p = fresh(d, &format!("s_{}", spec.replace(['+', '-', '=', ',', ' '], "_")), 0o600);
        let r = t::run("chmod", &[spec, &p], "");
        assert_eq!(r.code, 0, "spec {}: {}", spec, r.err());
        assert_eq!(mode_of(&p), want, "spec {}", spec);
    }
}

#[test]
fn changes_and_verbose() {
    let d = "/tmp/chmod_t3";
    let _ = std::fs::remove_dir_all(d);
    std::fs::create_dir_all(d).unwrap();
    let p = fresh(d, "f1", 0o600);
    let r = t::run("chmod", &["-c", "644", &p], "");
    assert_eq!(r.code, 0);
    assert_eq!(
        r.err().trim_end(),
        "mode of '<P>' changed from 0600 (rw-------) to 0644 (rw-r--r--)".replace("<P>", &p)
    );
    let r = t::run("chmod", &["-c", "644", &p], "");
    assert_eq!(r.code, 0);
    assert_eq!(r.err(), "", "-c 無變更時不應輸出");
    let r = t::run("chmod", &["-v", "644", &p], "");
    assert_eq!(
        r.err().trim_end(),
        format!("mode of '{}' retained as 0644 (rw-r--r--)", p)
    );
    let r = t::run("chmod", &["-v", "755", &p], "");
    assert_eq!(
        r.err().trim_end(),
        format!("mode of '{}' changed from 0644 (rw-r--r--) to 0755 (rwxr-xr-x)", p)
    );
}

#[test]
fn silent_keeps_rc() {
    let r = t::run("chmod", &["-f", "644", "/tmp/chmod_noexist_zz"], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err(), "");
    let r = t::run("chmod", &["644", "/tmp/chmod_noexist_zz"], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        "chmod: cannot access '/tmp/chmod_noexist_zz': No such file or directory"
    );
}

#[test]
fn recursive_and_reference() {
    let d = "/tmp/chmod_t4";
    let _ = std::fs::remove_dir_all(d);
    std::fs::create_dir_all(format!("{}/sub", d)).unwrap();
    let f = fresh(d, "ref", 0o640);
    std::fs::write(format!("{}/sub/g", d), b"y").unwrap();
    std::fs::set_permissions(format!("{}/sub/g", d), std::fs::Permissions::from_mode(0o777)).unwrap();
    let r = t::run("chmod", &["-R", "600", &format!("{}/sub", d)], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(mode_of(&format!("{}/sub", d)), 0o600);
    assert_eq!(mode_of(&format!("{}/sub/g", d)), 0o600);
    let r = t::run("chmod", &["--reference", &f, &format!("{}/sub", d)], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(mode_of(&format!("{}/sub", d)), 0o640);
}

#[test]
fn no_dereference_symlink() {
    let d = "/tmp/chmod_t5";
    let _ = std::fs::remove_dir_all(d);
    std::fs::create_dir_all(d).unwrap();
    let t1 = fresh(d, "tgt", 0o600);
    let sl = format!("{}/sl", d);
    std::os::unix::fs::symlink(&t1, &sl).unwrap();
    let r = t::run("chmod", &["-h", "640", &sl], "");
    // Linux 對 symlink lchmod 依檔案系統而異（tmpfs 可能 EOPNOTSUPP）；只要求 tgt 不變
    assert!(r.code == 0 || r.code == 1, "rc={}", r.code);
    assert_eq!(mode_of(&t1), 0o600, "-h 不應改目標");
}

#[test]
fn mode_errors() {
    let d = "/tmp/chmod_t6";
    let _ = std::fs::remove_dir_all(d);
    std::fs::create_dir_all(d).unwrap();
    let p = fresh(d, "f1", 0o600);
    let r = t::run("chmod", &["64z", &p], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), "chmod: invalid digit found in string");
    let r = t::run("chmod", &["a+z", &p], "");
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        "chmod: invalid operator (expected +, -, or =, but found z)"
    );
    let r = t::run("chmod", &["a", &p], "");
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), "chmod: invalid mode (a)");
    let r = t::run("chmod", &["-m", &p], "");
    assert_eq!(r.code, 2);
    let r = t::run("chmod", &[&p], "");
    assert_ne!(r.code, 0); // mode 缺失：把 operand 當 mode → 其後無檔
    let r = t::run("chmod", &["--help"], "");
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Usage"), "{}", r.out());
    let r = t::run("chmod", &["--version"], "");
    assert_eq!((r.code, r.out().trim_end()), (0, "chmod (uutils coreutils) 0.10.0"));
}

#[test]
fn fuzz_octal_roundtrip() {
    let d = "/tmp/chmod_fz";
    let _ = std::fs::remove_dir_all(d);
    std::fs::create_dir_all(d).unwrap();
    let mut rng = t::Rng::new(20261011);
    for i in 0..40 {
        let m = rng.below(0o10000) as u32;
        let spec = format!("{:04o}", m);
        let p = fresh(d, &format!("f{}", i), 0o000);
        let r = t::run("chmod", &[&spec, &p], "");
        assert_eq!(r.code, 0, "{}", r.err());
        assert_eq!(mode_of(&p), m, "spec {}", spec);
    }
}
