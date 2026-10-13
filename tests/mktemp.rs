//! mktemp(1) e2e tests（guest 內執行）。
//!
//! 基準：本機參考 uutils coreutils 0.10.0 pair-diff（c5align.py 電池：結構全一致，
//! 只差隨機字元本身）。隨機部分一律用「前綴／長度／字元集」驗證，不鎖定具體名稱。
use coreutils::testutil as t;
use std::os::unix::fs::PermissionsExt;

fn fresh(tag: &str) -> std::path::PathBuf {
    let d = std::path::PathBuf::from(format!("/tmp/mktemp_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn p(d: &std::path::Path, n: &str) -> String {
    d.join(n).to_string_lossy().into_owned()
}

fn m(args: &[&str]) -> t::Out {
    t::run("mktemp", args, "")
}

fn first_line(s: &str) -> String {
    s.lines().next().unwrap_or("").to_string()
}

fn is_rand(ch: char) -> bool {
    ch.is_ascii_alphanumeric()
}

/// 隨機段檢查：`name` 去掉已知前後綴後必須全是 [A-Za-z0-9]
fn assert_random(s: &str) {
    assert!(s.chars().all(is_rand), "non-alnum in random part: {:?}", s);
}

fn tmp_base() -> String {
    std::env::var("TMPDIR")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "/tmp".to_string())
}

#[test]
fn dry_run_default_template() {
    let _d = fresh("dry");
    let r = m(&["-u"]);
    assert_eq!(r.code, 0, "{}", r.err());
    let name = first_line(&r.out());
    assert!(name.starts_with(&format!("{}/tmp.", tmp_base())), "{}", name);
    let rand = name.rsplit('.').next().unwrap();
    assert_eq!(rand.len(), 10, "{}", name);
    assert_random(rand);
    // -u 不建立任何東西
    assert!(!std::path::Path::new(&name).exists());
    assert_eq!(r.out().lines().count(), 1);
}

#[test]
fn template_x_run_replacement() {
    let d = fresh("xrun");
    let r = m(&["-u", &p(&d, "aXXXXX")]);
    assert_eq!(r.code, 0, "{}", r.err());
    let name = first_line(&r.out());
    let base = std::path::Path::new(&name).file_name().unwrap().to_string_lossy().into_owned();
    assert!(base.starts_with("a"), "{}", base);
    assert_eq!(base.len(), 1 + 5, "{}", base);
    assert_random(&base[1..]);
    assert!(!std::path::Path::new(&name).exists());
    // X 在中間也要替換
    let r2 = m(&["-u", &p(&d, "bXXXXXc")]);
    assert_eq!(r2.code, 0, "{}", r2.err());
    let b2 = std::path::Path::new(&first_line(&r2.out()))
        .file_name()
        .unwrap()
        .to_string_lossy()
        .into_owned();
    assert!(b2.starts_with('b') && b2.ends_with('c') && b2.len() == 7, "{}", b2);
    assert_random(&b2[1..6]);
}

#[test]
fn template_without_x_needs_suffix() {
    let d = fresh("nosufx");
    // 參考實測：模板沒有 X 且沒給 --suffix → 錯誤（不是自動附加隨機字元）
    let bad = p(&d, "plain");
    let r = m(&["-u", &bad]);
    assert_eq!(r.code, 1, "{}", r.out());
    assert!(
        r.err()
            .starts_with(&format!("mktemp: too few X's in template '{}'", bad)),
        "{}",
        r.err()
    );

    let r2 = m(&["-u", "--suffix=.suf", &p(&d, "wu")]);
    assert_eq!(r2.code, 1, "{}", r2.out());
    assert_eq!(
        r2.err().trim_end(),
        format!("mktemp: with --suffix, template '{}' must end in X", p(&d, "wu"))
    );
    // 有 X run 但不在結尾 → 同樣拒絕
    let r3 = m(&["-u", "--suffix=.s", "aXXXXb"]);
    assert_eq!(r3.code, 1);
    assert_eq!(
        r3.err().trim_end(),
        "mktemp: with --suffix, template 'aXXXXb' must end in X"
    );
    // X run 在結尾 → suffix 接在其後
    let r4 = m(&["-u", "--suffix=.s", "wuXXXX"]);
    assert_eq!(r4.code, 0, "{}", r4.err());
    let n4 = first_line(&r4.out());
    assert!(n4.starts_with("wu") && n4.ends_with(".s"), "{}", n4);
    let mid4 = &n4[2..n4.len() - 2];
    assert_random(mid4);
}

#[test]
fn explicit_suffix_after_x_run() {
    let d = fresh("suf");
    let r = m(&["-u", "--suffix=.suf", &p(&d, "aa.XXXX")]);
    assert_eq!(r.code, 0, "{}", r.err());
    let name = first_line(&r.out());
    let base = std::path::Path::new(&name).file_name().unwrap().to_string_lossy().into_owned();
    assert!(base.starts_with("aa."), "{}", base);
    assert!(base.ends_with(".suf"), "{}", base);
    let mid = &base[3..base.len() - 4];
    assert_eq!(mid.len(), 4, "{}", base);
    assert_random(mid);

    let e = m(&["--suffix", "/x", &p(&d, "bad1")]);
    assert_eq!(e.code, 1);
    assert!(
        e.err()
            .starts_with("mktemp: invalid suffix '/x', contains directory separator"),
        "{}",
        e.err()
    );
}

#[test]
fn too_few_x_errors() {
    // 無 X 的模板（未給 --suffix）→ 錯誤（參考實測）
    let r = m(&["-u", "plain_no_x_unused"]);
    assert_eq!(r.code, 1, "{}", r.out());
    assert!(
        r.err()
            .starts_with("mktemp: too few X's in template 'plain_no_x_unused'"),
        "{}",
        r.err()
    );
    // 注：無分隔符的模板也一樣（參考實測）；錯誤只有這一行的訊息，無 Try 行。
    let e = m(&["-u", "-t", "sub/dir.X"]);
    assert_eq!(e.code, 1);
    assert!(
        e.err()
            .starts_with("mktemp: too few X's in template 'sub/dir.X'"),
        "{}",
        e.err()
    );
    let f = m(&["-u", "--tmpdir", "sub.X"]);
    assert_eq!(f.code, 1);
    assert!(
        f.err()
            .starts_with("mktemp: too few X's in template 'sub.X'"),
        "{}",
        f.err()
    );
}

#[test]
fn too_many_templates() {
    let r = m(&["-u", "aXXXXX_unused", "bXXXXX_unused"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with("mktemp: too many templates"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("Try '"), "{}", r.err());
}

#[test]
fn t_flag_uses_tmpdir_prefix() {
    let tag = format!("t_{}", std::process::id());
    let dir = fresh(&tag);
    std::env::set_var("TMPDIR", &dir);
    let r = m(&["-u", "-t", "ex.XXXXXX"]);
    assert_eq!(r.code, 0, "{}", r.err());
    let name = first_line(&r.out());
    assert!(
        name.starts_with(&format!("{}/ex.", dir.display())),
        "{} (TMPDIR={})",
        name,
        dir.display()
    );
    let rand = name.rsplit('.').next().unwrap();
    assert_eq!(rand.len(), 6, "{}", name);
    assert_random(rand);
    std::env::remove_var("TMPDIR");
}

#[test]
fn tmpdir_and_p_forms() {
    let _d = fresh("tmpdir");
    std::env::set_var("TMPDIR", "");
    let r = m(&["-u", "--tmpdir=sub_noop_unused", "q.XXXXXX_unused"]);
    // 參考實測：--tmpdir 以 `=` 給值才吞值；空白分隔的下一個 token 當作 operand →
    // 兩個 operand → "too many templates"（帶 Try 行）。
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(
        first_line(&r.out()).contains("sub_noop_unused/q."),
        "{}",
        r.out()
    );
    let r2 = m(&["-u", "--tmpdir", "sep_noop_unused", "q.XXXXXX_unused"]);
    assert_eq!(r2.code, 1);
    assert!(r2.err().contains("too many templates"), "{}", r2.err());
    let e = m(&["-p", "/nonexistent_dir_for_test", "q.XXXXXX_unused"]);
    assert_eq!(e.code, 1);
    assert!(
        e.err().starts_with(
            "mktemp: failed to create file via template '/nonexistent_dir_for_test/q.XXXXXX_unused'"
        ),
        "{}",
        e.err()
    );
    assert!(e.err().contains("No such file or directory"), "{}", e.err());
    // -q 靜音但退出碼不變
    let q = m(&["-q", "-p", "/nonexistent_dir_for_test", "q.XXXXXX_unused"]);
    assert_eq!(q.code, 1);
    assert_eq!(q.err(), "");
    std::env::remove_var("TMPDIR");
}

#[test]
fn creates_file_with_private_mode() {
    let d = fresh("create");
    let tmpl = p(&d, "f.XXXX");
    let r = m(&[&tmpl]);
    assert_eq!(r.code, 0, "{}", r.err());
    let name = first_line(&r.out());
    let md = std::fs::metadata(&name).unwrap();
    assert!(md.is_file());
    assert_eq!(md.permissions().mode() & 0o777, 0o600);
    assert_eq!(md.len(), 0);
}

#[test]
fn creates_directory_with_dash_d() {
    let d = fresh("dashd");
    let tmpl = p(&d, "ddir.XXXX");
    let r = m(&["-d", &tmpl]);
    assert_eq!(r.code, 0, "{}", r.err());
    let name = first_line(&r.out());
    let md = std::fs::metadata(&name).unwrap();
    assert!(md.is_dir());
    assert_eq!(md.permissions().mode() & 0o777, 0o700);
    // -u -d 只印名字
    let r2 = m(&["-u", "-d", &p(&d, "ddir2.XXXX")]);
    assert_eq!(r2.code, 0, "{}", r2.err());
    assert!(!std::path::Path::new(&first_line(&r2.out())).exists());
}

#[test]
fn option_needs_value_and_unknown_option() {
    let r = m(&["-p"]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("a value is required for '-p <DIR>'"), "{}", r.err());
    assert!(r.err().contains("For more information, try '--help'."), "{}", r.err());
    let e = m(&["aXXXXX_unused", "-p"]);
    assert_eq!(e.code, 1);
}

#[test]
fn help_and_version() {
    let h = m(&["--help"]);
    assert_eq!(h.code, 0);
    assert!(
        h.out()
            .starts_with("Create a temporary file or directory.\n\nUsage: mktemp [OPTION]... [TEMPLATE]\n"),
        "{}",
        h.out()
    );
    assert!(h.out().contains("Arguments:\n  [template]"), "{}", h.out());
    assert!(
        h.out()
            .contains("  -d, --directory        Make a directory instead of a file"),
        "{}",
        h.out()
    );
    assert!(h.out().contains("  -p <DIR>               short form of --tmpdir"), "{}", h.out());
    assert!(h.out().contains("[deprecated]"), "{}", h.out());
    let v = m(&["--version"]);
    assert_eq!(v.code, 0);
    assert!(v.out().contains("coreutils"), "{}", v.out());
    assert!(m(&["-hV"]).out().contains("Usage: mktemp"));
    assert!(m(&["-Vh"]).out().contains("coreutils"));
    assert!(m(&["--hel"]).out().contains("Usage: mktemp"));
}

#[test]
fn names_are_unique_and_charset() {
    let _d = fresh("uniq");
    let mut seen = std::collections::HashSet::new();
    for _ in 0..200 {
        let r = m(&["-u"]);
        assert_eq!(r.code, 0, "{}", r.err());
        let name = first_line(&r.out());
        let rand = name.rsplit('.').next().unwrap();
        assert_eq!(rand.len(), 10);
        assert_random(rand);
        assert!(seen.insert(name), "duplicate name generated");
    }
    assert_eq!(seen.len(), 200);
}

#[test]
fn run_bin_dispatch() {
    let d = fresh("bin");
    let tmpl = p(&d, "b.XXX");
    let r = t::run_bin("mktemp", &[&tmpl], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(std::path::Path::new(&first_line(&r.out())).exists());
    let h = t::run_bin("mktemp", &["--help"], "");
    assert_eq!(h.code, 0);
    assert!(h.out().contains("Create a temporary file or directory."));
}

#[test]
fn fuzz_invariants() {
    // 不變量：任意 -t/-p 模板組合都建立唯一路徑（每次不同、確實存在、權限 0600）；
    // 非法模板（無 XXX）→ rc1 + man 措辭、不建立檔案。
    let dir = fresh("fuzz");
    let mut st: u64 = 0x7E570D0D;
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    for _ in 0..40u64 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let use_p = (st >> 50) % 2 == 0;
        let stem = format!("t{:04x}", (st >> 12) % 65536);
        let (args, expect_dir) = if use_p {
            (vec!["-p".to_string(), dir.display().to_string(), format!("{stem}XXXXXX")], dir.clone())
        } else {
            (vec![format!("{}/{}XXXXXX", dir.display(), stem)], dir.clone())
        };
        let argsv: Vec<&str> = args.iter().map(|s: &String| s.as_str()).collect();
        let r = t::run("mktemp", &argsv, "");
        assert_eq!(r.code, 0, "{stem} {}", r.err());
        let out = r.out().trim().to_string();
        assert!(!seen.contains(&out), "duplicate {out}");
        seen.insert(out.clone());
        let p = std::path::PathBuf::from(&out);
        let np = if p.is_relative() { expect_dir.join(&out) } else { p };
        assert!(np.exists(), "{np:?}");
        let mode = std::fs::metadata(&np).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600, "{out} mode {mode:o}");
    }
    let bad = t::run("mktemp", &["-t", "NOPLACEHOLDER"], "");
    assert_eq!(bad.code, 1);
    assert!(bad.err().contains("too few X's in template 'NOPLACEHOLDER'"), "{}", bad.err());
}
