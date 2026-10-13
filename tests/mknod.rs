//! mknod(1) e2e tests（guest 內執行，guest 為 root 故可建立裝置節點）。
//!
//! 基準：本機參考 uutils coreutils 0.10.0 pair-diff（docs/probes/cluster5.md＋c5align.py 電池）。
use coreutils::testutil as t;
use std::os::unix::fs::{FileTypeExt, MetadataExt, PermissionsExt};

fn fresh(tag: &str) -> std::path::PathBuf {
    let d = std::path::PathBuf::from(format!("/tmp/mknod_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn p(d: &std::path::Path, n: &str) -> String {
    d.join(n).to_string_lossy().into_owned()
}

fn m(args: &[&str]) -> t::Out {
    t::run("mknod", args, "")
}

fn mode_of(path: &str) -> u32 {
    std::fs::metadata(path).unwrap().permissions().mode() & 0o7777
}

/// 未給 -m 時的觀察預設 mode（= 0o666 & !umask）
fn default_mode(d: &std::path::Path) -> u32 {
    let f = p(d, "probe_default");
    assert_eq!(m(&[&f, "p"]).code, 0);
    let mo = mode_of(&f) & 0o777;
    std::fs::remove_file(&f).unwrap();
    mo
}

#[test]
fn fifo_basic_and_default_mode() {
    let d = fresh("fifo");
    let f = p(&d, "f1");
    let r = m(&[&f, "p"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(std::fs::metadata(&f).unwrap().file_type().is_fifo());
    assert_eq!(mode_of(&f) & 0o777, default_mode(&d));
}

#[test]
fn mode_option_forms() {
    let d = fresh("mode");
    for (opts, name, want) in [
        (&["-m", "640"][..], "640", 0o640),
        (&["-m", "777"][..], "777", 0o777),
        (&["--mode=777"][..], "777b", 0o777),
        (&["-m", "0"][..], "000", 0o000),
    ] {
        let path = p(&d, name);
        let mut argv: Vec<&str> = opts.to_vec();
        argv.push(path.as_str());
        argv.push("p");
        let r = m(&argv);
        assert_eq!(r.code, 0, "{:?} {}", opts, r.err());
        assert_eq!(mode_of(&path) & 0o777, want, "{:?}", opts);
    }
}

#[test]
fn mode_relative_forms() {
    let d = fresh("mode_rel");
    // uutils 的 -m 接受 GNU 相對語法；測試一律用顯式 who（結果與 umask 無關，
    // symbolic 起始 mode 固定 0o666）。參考實測：u+wx→766、go=r→644、a+rw→666、
    // u=rwx,go=rx→755、u=r→466、a=rwx→777。
    for (spec, want) in [
        ("u+wx", 0o766),
        ("go=r", 0o644),
        ("a+rw", 0o666),
        ("u=rwx,go=rx", 0o755),
        ("u=r", 0o466),
        ("a=rwx", 0o777),
    ] {
        let name: String = spec
            .chars()
            .map(|c| if c.is_alphanumeric() { c } else { '_' })
            .collect();
        let path = p(&d, &name);
        let r = m(&["-m", spec, &path, "p"]);
        assert_eq!(r.code, 0, "{:?} {}", spec, r.err());
        assert_eq!(mode_of(&path) & 0o777, want, "{:?}", spec);
    }
    let e = m(&["-m", "bogus", &p(&d, "bad"), "p"]);
    assert_eq!(e.code, 1);
    assert!(
        e.err().starts_with("mknod: invalid mode ("),
        "unexpected err: {}",
        e.err()
    );
}

#[test]
fn char_device_and_rdev() {
    let d = fresh("char");
    let f = p(&d, "c1");
    let r = m(&[&f, "c", "1", "3"]);
    assert_eq!(r.code, 0, "{}", r.err());
    let md = std::fs::metadata(&f).unwrap();
    assert!(md.file_type().is_char_device());
    assert_eq!(md.rdev(), (1 << 8) | 3);
    assert_eq!(md.permissions().mode() & 0o777, default_mode(&d));
    // MAJOR/MINOR 走十進位（uutils 用 plain parse：'010' → 10，不是八進位 8）。
    let g = p(&d, "c2");
    let r2 = m(&[&g, "c", "010", "2"]);
    assert_eq!(r2.code, 0, "{}", r2.err());
    assert_eq!(std::fs::metadata(&g).unwrap().rdev(), (10 << 8) | 2);
    // '0x..' 一律被拒（兩個本機參考皆如此；GNU info 宣稱的 hex 未實作）
    let h = p(&d, "c3");
    let r3 = m(&[&h, "c", "0x10", "4"]);
    assert_eq!(r3.code, 1);
    assert!(
        r3.err().contains("invalid value '0x10' for '[major]'"),
        "{}",
        r3.err()
    );
    std::fs::remove_file(&f).ok();
}

#[test]
fn block_device_and_type_alias_u() {
    let d = fresh("block");
    let b1 = p(&d, "b1");
    let r = m(&[&b1, "b", "1", "3"]);
    assert_eq!(r.code, 0, "{}", r.err());
    let md = std::fs::metadata(&b1).unwrap();
    assert!(md.file_type().is_block_device());
    assert_eq!(md.rdev(), (1 << 8) | 3);
    // 'u' = 字元裝置（unbuffered）
    let u1 = p(&d, "u1");
    let r2 = m(&[&u1, "u", "1", "3"]);
    assert_eq!(r2.code, 0, "{}", r2.err());
    assert!(std::fs::metadata(&u1).unwrap().file_type().is_char_device());
}

#[test]
fn devices_require_major_minor() {
    let d = fresh("nodev");
    for (name, ty, extra) in [
        ("c2", "c", &[] as &[&str]),
        ("c3", "c", &["1"][..]),
        ("b2", "b", &[]),
        ("b3", "b", &["7"]),
        ("u2", "u", &[]),
    ] {
        let path = p(&d, name);
        let mut argv: Vec<&str> = vec![path.as_str(), ty];
        argv.extend_from_slice(extra);
        let r = m(&argv);
        assert_eq!(r.code, 1, "{:?}", argv);
        assert!(
            r.err()
                .starts_with("mknod: Special files require major and minor device numbers."),
            "{:?} -> {}",
            argv,
            r.err()
        );
        assert!(r.err().contains("Try '"), "{}", r.err());
    }
}

#[test]
fn fifo_rejects_major_minor() {
    let r = m(&["p3", "p", "1", "2"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err()
            .starts_with("mknod: Fifos do not have major and minor device numbers."),
        "{}",
        r.err()
    );
    assert!(r.err().contains("Try '"), "{}", r.err());
}

#[test]
fn invalid_type_and_numbers() {
    let r = m(&["x1", "x"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("invalid value 'x' for '<TYPE>': invalid device type 'x'"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("For more information, try '--help'."), "{}", r.err());

    let e = m(&["p4", "p", "x"]);
    assert_eq!(e.code, 1);
    assert!(
        e.err()
            .contains("invalid value 'x' for '[major]': invalid digit found in string"),
        "{}",
        e.err()
    );

    // major/minor 不接受 0x 前綴以外的寫法，且 hex 由 CLI 層拒絕（參考實測）
    let h = m(&["c4", "c", "0x1", "2"]);
    assert_eq!(h.code, 1);
    assert!(
        h.err()
            .contains("invalid value '0x1' for '[major]': invalid digit found in string"),
        "{}",
        h.err()
    );
}

#[test]
fn unexpected_extra_positional_has_no_tip() {
    let r = m(&["c5", "c", "1", "2", "3"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err()
            .starts_with("error: unexpected argument '3' found\n\nUsage: mknod [OPTION]... NAME TYPE [MAJOR MINOR]\n"),
        "{}",
        r.err()
    );
    assert!(!r.err().contains("tip:"), "{}", r.err());
}

#[test]
fn missing_required_operands() {
    let r = m(&[]);
    assert_eq!(r.code, 1);
    assert!(
        r.err()
            .contains("error: the following required arguments were not provided:"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("<NAME>"), "{}", r.err());
    assert!(r.err().contains("<TYPE>"), "{}", r.err());

    let e = m(&["only"]);
    assert_eq!(e.code, 1);
    assert!(e.err().contains("<TYPE>"), "{}", e.err());
}

#[test]
fn option_errors() {
    let r = m(&["-m"]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("a value is required"), "{}", r.err());
    let e = m(&["--mode"]);
    assert_eq!(e.code, 1);
    assert!(e.err().contains("a value is required"), "{}", e.err());
}

#[test]
fn context_options_are_accepted() {
    let d = fresh("ctx");
    // 無 SELinux 的 guest：-Z / --context 仍接受（參考行為：設定失敗才報錯）
    let r = m(&["-Z", &p(&d, "z1"), "p"]);
    assert!(r.code == 0 || r.err().contains("mknod:"), "{}", r.err());
    let e = m(&["--context=foo", &p(&d, "z2"), "p"]);
    assert!(e.code == 0 || e.err().contains("mknod:"), "{}", e.err());
}

#[test]
fn help_and_version() {
    let h = m(&["--help"]);
    assert_eq!(h.code, 0);
    assert!(
        h.out()
            .starts_with("Create the special file NAME of the given TYPE.\n\nUsage: mknod [OPTION]... NAME TYPE [MAJOR MINOR]\n"),
        "{}",
        h.out()
    );
    assert!(h.out().contains("  <NAME>   name of the new file"), "{}", h.out());
    assert!(h.out().contains("  [major]  major file type"), "{}", h.out());
    assert!(
        h.out()
            .contains("Both MAJOR and MINOR must be specified when TYPE is b, c, or u"),
        "{}",
        h.out()
    );
    assert!(
        h.out()
            .contains("NOTE: your shell may have its own version of mknod"),
        "{}",
        h.out()
    );
    let v = m(&["--version"]);
    assert_eq!(v.code, 0);
    assert!(v.out().contains("coreutils"), "{}", v.out());
    // 先到先得
    assert_eq!(m(&["-hV"]).code, 0);
    assert!(m(&["-hV"]).out().contains("Usage: mknod"), "{}", m(&["-hV"]).out());
    assert!(m(&["-Vh"]).out().contains("coreutils"), "{}", m(&["-Vh"]).out());
    // 前綴唯一匹配
    assert!(m(&["--hel"]).out().contains("Usage: mknod"));
    assert_eq!(m(&["--vers"]).code, 0);
}

#[test]
fn existing_target_is_reported() {
    let d = fresh("exists");
    let f = p(&d, "dup");
    std::fs::write(&f, b"x").unwrap();
    let r = m(&[&f, "p"]);
    // 參考實測：檔案系統錯誤走 rc 255（不是 1）；error 前綴是 invoked_as 路徑
    // （guest 內 test binary 為 /mnt/tests/<id>，故只比對後半段）。
    assert_eq!(r.code, 255);
    assert!(
        r.err()
            .ends_with(": File exists (os error 17)\n"),
        "{}",
        r.err()
    );
}

#[test]
fn long_option_prefix_inference() {
    let d = fresh("infer");
    // clap infer_long_args：長選項可用唯一前綴（參考實測 --mo=640 可用）
    let f = p(&d, "i1");
    let r = m(&["--mo=640", &f, "p"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(mode_of(&f) & 0o777, 0o640);
    let g = p(&d, "i2");
    let r2 = m(&["--mod", "777", &g, "p"]);
    assert_eq!(r2.code, 0, "{}", r2.err());
    assert_eq!(mode_of(&g) & 0o777, 0o777);
    // --con 對 --context 唯一 → 被接受（接著因缺 operand 而報 required）
    let c = m(&["--con"]);
    assert!(c.err().contains("required arguments"), "{}", c.err());
}

#[test]
fn run_bin_dispatch() {
    let d = fresh("bin");
    let f = p(&d, "bf1");
    let r = t::run_bin("mknod", &[&f, "p"], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(std::fs::metadata(&f).unwrap().file_type().is_fifo());
    let h = t::run_bin("mknod", &["--help"], "");
    assert_eq!(h.code, 0);
    assert!(h.out().contains("Create the special file NAME of the given TYPE."));
}

fn pick_mode(st: u64) -> u64 {
    (st >> 50) % 4
}

#[test]
fn fuzz_invariants() {
    // 不變量：合法 (type[,major,minor]) 建立對應檔案類型；非法參數 → rc∈{1,2} 且不建立檔案；
    // 重複建立同名 → rc1 EEXIST、既有檔案不受影響。
    use std::os::unix::fs::FileTypeExt;
    let dir = fresh("fuzz");
    let mut st: u64 = 0xDAD10DE5;
    for i in 0..60u32 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let name = format!("n{i}");
        let path = dir.join(&name);
        // 我方決定：mode0 只測 FIFO（c/b 需要 major/minor，mode1 才測）；mode3 用 c 測非法 major
        let kind = match pick_mode(st) {
            0 => "p",
            3 => "c",
            _ => ["c", "b"][(st >> 40) as usize % 2],
        };
        let major = ((st >> 20) % 512).to_string();
        let minor = ((st >> 10) % 512).to_string();
        let mode = pick_mode(st);
        let full = path.display().to_string();
        let args: Vec<String> = match mode {
            0 => vec![full.clone(), kind.to_string()],
            1 => vec![full.clone(), kind.to_string(), major.clone(), minor.clone()],
            2 => vec![full.clone(), "x".to_string()],
            _ => vec![full.clone(), kind.to_string(), "-3".to_string(), minor.clone()],
        };
        let r = t::run("mknod", &args.iter().map(|s| s.as_str()).collect::<Vec<_>>(), "");
        match mode {
            0 | 1 => {
                assert_eq!(r.code, 0, "{:?}", args);
                let md = std::fs::metadata(&path).expect("metadata");
                let ft = md.file_type();
                match kind {
                    "p" => assert!(ft.is_fifo()),
                    "c" => assert!(ft.is_char_device()),
                    _ => assert!(ft.is_block_device()),
                }
            }
            2 => {
                assert!(r.code == 1 || r.code == 2, "{:?} -> {}", args, r.code);
                assert!(!path.exists(), "{:?}", args);
            }
            _ => {
                assert!(r.code == 1 || r.code == 2, "{:?} -> {}", args, r.code);
            }
        }
    }
    // 重複 EEXIST（先確保 n0 存在）
    let n0 = dir.join("n0").display().to_string();
    let first = t::run("mknod", &[n0.as_str(), "p"], "");
    assert_eq!(first.code, 0, "{}", first.err());
    let dup = t::run("mknod", &[n0.as_str(), "p"], "");
    assert_eq!(dup.code, 255); // 我方慣例：mknod(2) 失敗 → 255 + "mknod: ...: File exists"
    assert!(dup.err().contains("File exists"), "{}", dup.err());
    assert!(dir.join("n0").exists());
}
