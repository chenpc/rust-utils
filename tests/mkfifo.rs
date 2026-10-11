//! mkfifo e2e（guest 內執行）
//! 行為規格：docs/man/mkfifo.1.gz（GNU：`mkfifo [OPTION]... NAME...`、`-m MODE`、`-Z`、`--context`）
//! ＋本專案已定案的 clap 措辭（man 未載的措辭記於 docs/assumptions.md）。
use coreutils::testutil as t;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::path::{Path, PathBuf};

fn fresh(tag: &str) -> PathBuf {
    let d = PathBuf::from(format!("/tmp/mkfifo_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).ok();
    d
}

fn m(args: &[&str]) -> t::Out {
    t::run("mkfifo", args, "")
}

fn mode_of(p: &Path) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o7777
}

fn is_fifo(p: &Path) -> bool {
    std::fs::symlink_metadata(p).unwrap().file_type().is_fifo()
}

/// 以 0666 建立一個檔案反推目前 umask（mkfifo 預設 mode = 0666 & ~umask）
fn default_mode(d: &Path) -> u32 {
    let probe = d.join("umask_probe");
    std::fs::write(&probe, b"").unwrap();
    let got = mode_of(&probe);
    0o666 & !(!got & 0o7777)
}

#[test]
fn creates_fifo_with_default_mode() {
    let d = fresh("def");
    let expect = default_mode(&d);
    let p = d.join("f1");
    let r = m(&[p.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.err(), "");
    assert!(is_fifo(&p), "必須是 FIFO");
    assert_eq!(mode_of(&p), expect, "預設 mode 須為 0666 & ~umask");
}

#[test]
fn mode_octal_forms() {
    let d = fresh("oct");
    for (spec, want) in [
        ("600", 0o600u32),
        ("777", 0o777),
        ("0", 0o000),
        ("0640", 0o640),
        ("000", 0o000),
    ] {
        let p = d.join(format!("o{}", spec));
        let r = m(&["-m", spec, p.to_str().unwrap()]);
        assert_eq!(r.code, 0, "-m {}: {}", spec, r.err());
        assert_eq!(mode_of(&p), want, "-m {}", spec);
        assert!(is_fifo(&p));

        let p2 = d.join(format!("l{}", spec));
        let r = m(&[&format!("--mode={}", spec), p2.to_str().unwrap()]);
        assert_eq!(r.code, 0, "--mode={}: {}", spec, r.err());
        assert_eq!(mode_of(&p2), want, "--mode={}", spec);

        let p3 = d.join(format!("a{}", spec));
        let r = m(&[&format!("-m={}", spec), p3.to_str().unwrap()]);
        assert_eq!(r.code, 0, "-m={}: {}", spec, r.err());
        assert_eq!(mode_of(&p3), want, "-m={}", spec);
    }
}

#[test]
fn mode_symbolic_base_is_0666() {
    let d = fresh("sym");
    // man：-m 的符號模式以 umask 後的 mode 為基底；本專案定案基底為 0666（見 docs/assumptions.md）
    for (spec, want) in [
        ("a+x", 0o777u32),
        ("u=rwx,go=rx", 0o755),
        ("go=r", 0o644),
        ("u+r", 0o666),
        ("g-w", 0o646),
        ("640,a+x", 0o751),
    ] {
        let p = d.join(format!("s{}", spec.replace(['+', ',', '=', '-'], "_")));
        let r = m(&["-m", spec, p.to_str().unwrap()]);
        assert_eq!(r.code, 0, "-m {}: {}", spec, r.err());
        assert_eq!(mode_of(&p), want, "-m {}", spec);
    }
}

#[test]
fn invalid_mode_message() {
    let d = fresh("bad");
    let p = d.join("f");
    let r = m(&["-m", "bogus", p.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        "mkfifo: invalid mode: invalid operator (expected +, -, or =, but found b)"
    );
    assert!(!p.exists(), "mode 錯誤時不得建立檔案");

    // 缺 NAME 時 clap 先報缺少 positional（mode 錯誤延後）
    let r = m(&["-m", "bogus"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err()
            .starts_with("error: the following required arguments were not provided:"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("  <fifo>..."));
    assert!(r.err().contains("Usage: mkfifo [OPTION]... NAME..."));
    assert!(r.err().contains("For more information, try '--help'."));
}

#[test]
fn missing_value_for_mode_option() {
    for (flag, shown) in [("-m", "--mode <MODE>"), ("--mode", "--mode <MODE>")] {
        let r = m(&[flag]);
        assert_eq!(r.code, 1, "{}", flag);
        assert_eq!(
            r.err().trim_end(),
            format!(
                "error: a value is required for '{}' but none was supplied\n\n\
                 For more information, try '--help'.",
                shown
            ),
            "{}",
            flag
        );
    }
}

#[test]
fn no_operand_reports_required_argument() {
    let r = m(&[]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: the following required arguments were not provided:\n  <fifo>...\n\n\
         Usage: mkfifo [OPTION]... NAME...\n\n\
         For more information, try '--help'.\n"
    );
}

#[test]
fn unknown_options_use_clap_shape() {
    let r = m(&["--bogus", "x"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err(),
        "error: unexpected argument '--bogus' found\n\n  \
         tip: to pass '--bogus' as a value, use '-- --bogus'\n\n\
         Usage: mkfifo [OPTION]... NAME...\n\n\
         For more information, try '--help'.\n"
    );

    let r = m(&["-x", "x"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with("error: unexpected argument '-x' found"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("  tip: to pass '-x' as a value, use '-- -x'"));
    assert!(r.err().contains("Usage: mkfifo [OPTION]... NAME..."));

    // 長選項前綴推斷（clap infer_long_args）：--mo -> --mode（需值）
    let r = m(&["--mo"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("a value is required for '--mode <MODE>'"),
        "{}",
        r.err()
    );
    // 無匹配的前綴 → unexpected + tip
    let r = m(&["--fo"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with("error: unexpected argument '--fo' found"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("  tip: to pass '--fo' as a value, use '-- --fo'"));
}

#[test]
fn existing_name_and_missing_directory() {
    let d = fresh("dup");
    let p = d.join("f");
    assert_eq!(m(&[p.to_str().unwrap()]).code, 0);
    let r = m(&[p.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        format!("mkfifo: cannot create fifo '{}': File exists", p.to_string_lossy())
    );

    let missing = d.join("no_dir/f");
    let r = m(&[missing.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        format!(
            "mkfifo: cannot create fifo '{}': No such file or directory",
            missing.to_string_lossy()
        )
    );

    // 多個 NAME：部分失敗仍處理全部，rc=1
    let a = d.join("m1");
    let b = d.join("m2");
    assert_eq!(m(&[a.to_str().unwrap()]).code, 0);
    let r = m(&[a.to_str().unwrap(), b.to_str().unwrap()]);
    assert_eq!(r.code, 1);
    assert!(is_fifo(&b), "第二個 NAME 仍須建立");
    assert!(r.err().contains("File exists"));
}

#[test]
fn selinux_options_are_accepted() {
    let d = fresh("sel");
    // man 有 -Z 與 --context[=CTX]；guest 無 SELinux → 接受但不影響建立
    let p = d.join("z1");
    let r = m(&["-Z", p.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(is_fifo(&p));

    let p = d.join("z2");
    let r = m(&["--context=foo", p.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(is_fifo(&p));

    let p = d.join("z3");
    let r = m(&["-Zfoo", p.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(is_fifo(&p));
}

#[test]
fn help_and_version() {
    let r = m(&["--help"]);
    assert_eq!(r.code, 0);
    let h = r.out();
    assert_eq!(h.lines().next().unwrap(), "Create a FIFO with the given name.");
    assert!(h.contains("Usage: mkfifo [OPTION]... NAME..."));
    for s in [
        "  -m, --mode <MODE>      file permissions for the fifo",
        "  -Z                     set the SELinux security context to default type",
        "  -h, --help             Print help",
        "  -V, --version          Print version",
    ] {
        assert!(h.contains(s), "help 須含 {:?}", s);
    }

    let r = m(&["-V"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out().trim_end(), "mkfifo (uutils coreutils) 0.10.0");

    // 先到先得：-hV -> help、-Vh -> version
    assert!(m(&["-hV"]).out().contains("Create a FIFO"));
    assert!(m(&["-Vh"]).out().contains("0.10.0"));
    // 長選項前綴：--hel -> --help、--vers -> --version
    assert!(m(&["--hel"]).out().contains("Create a FIFO"));
    assert_eq!(m(&["--vers"]).out().trim_end(), "mkfifo (uutils coreutils) 0.10.0");
}

#[test]
fn multiple_names_and_dispatch() {
    let d = fresh("multi");
    let names: Vec<String> = (0..4).map(|i| d.join(format!("n{}", i)).to_string_lossy().into_owned()).collect();
    let refs: Vec<&str> = names.iter().map(|s| s.as_str()).collect();
    let r = m(&refs.as_slice());
    assert_eq!(r.code, 0, "{}", r.err());
    for n in &names {
        let p = Path::new(n);
        assert!(is_fifo(p) && mode_of(p) == default_mode(&d), "{}", n);
    }

    let r = t::run_bin("mkfifo", &[d.join("b1").to_str().unwrap()], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(is_fifo(&d.join("b1")));
}

#[test]
fn fuzz_names_modes_and_perms() {
    let mut rng = t::Rng::new(20261120);
    let d = fresh("fz");
    let base = default_mode(&d);
    for k in 0..16 {
        let name = format!("fz{}", k);
        let p = d.join(&name);
        let use_mode = rng.below(3) != 0;
        let octal = format!("{}{}{}", rng.below(8), rng.below(8), rng.below(8));
        let mut args: Vec<String> = Vec::new();
        if use_mode {
            args.push("-m".to_string());
            args.push(octal.clone());
        }
        args.push(p.to_string_lossy().into_owned());
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let r = m(&refs);
        assert_eq!(r.code, 0, "case {}: {}", k, r.err());
        assert!(is_fifo(&p), "case {}", k);
        let want = if use_mode {
            u32::from_str_radix(&octal, 8).unwrap()
        } else {
            base
        };
        assert_eq!(mode_of(&p), want, "case {} mode（-m {}）", k, octal);
    }
}
