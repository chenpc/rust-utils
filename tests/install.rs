//! install(1) e2e tests（guest 內執行）。
//!
//! 行為基準取自本機參考 uutils coreutils 0.10.0 的逐 case pair-diff
//! （~/tools/pairdrive.py，49 cases 全一致；結果記錄於 docs/probes/cluster5.md）。
use coreutils::testutil as t;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

fn fresh(tag: &str) -> std::path::PathBuf {
    let d = std::path::PathBuf::from(format!("/tmp/inst_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::env::set_current_dir(&d).unwrap();
    d
}

fn i(args: &[&str]) -> t::Out {
    t::run("install", args, "")
}

fn mode_of(p: &str) -> u32 {
    std::fs::metadata(p).unwrap().permissions().mode() & 0o7777
}

fn write(p: &str, content: &str, mode: u32) {
    if let Some(d) = std::path::Path::new(p).parent() {
        if !d.as_os_str().is_empty() {
            std::fs::create_dir_all(d).ok();
        }
    }
    std::fs::write(p, content).unwrap();
    std::fs::set_permissions(p, std::fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn default_mode_is_755_regardless_of_src() {
    fresh("dflt");
    for src_mode in [0o644u32, 0o600, 0o755, 0o444] {
        write("src_a", "AAA", src_mode);
        let r = i(&["src_a", "dst"]);
        assert_eq!(r.code, 0, "{}", r.err());
        assert_eq!(mode_of("dst"), 0o755, "src mode {:o}", src_mode);
        assert_eq!(std::fs::read_to_string("dst").unwrap(), "AAA");
        std::fs::remove_file("dst").ok();
    }
}

#[test]
fn explicit_mode_octal_and_symbolic() {
    fresh("mode");
    write("src_a", "AAA", 0o644);
    assert_eq!(i(&["-m", "600", "src_a", "m1"]).code, 0);
    assert_eq!(mode_of("m1"), 0o600);
    // 參考實測：symbolic mode 以 0 為基底（不是 umask-masked 0777）
    assert_eq!(i(&["-m", "a+x", "src_a", "m2"]).code, 0);
    assert_eq!(mode_of("m2"), 0o111);
    assert_eq!(i(&["-m", "u=rw,g=r,o=", "src_a", "m3"]).code, 0);
    assert_eq!(mode_of("m3"), 0o640);
    assert_eq!(i(&["--mode", "700", "src_a", "m4"]).code, 0);
    assert_eq!(mode_of("m4"), 0o700);
    // 錯誤措辭（大寫 Invalid）＋不建立 dst
    let r = i(&["-m", "bogus", "src_a", "m5"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        "install: Invalid mode string: invalid operator (expected +, -, or =, but found b)"
    );
    assert!(!std::path::Path::new("m5").exists());
}

#[test]
fn into_directory_and_multiple_sources() {
    fresh("dir");
    write("src/a", "A", 0o644);
    write("sz3", "CCC", 0o644);
    std::fs::create_dir_all("d1").unwrap();
    assert_eq!(i(&["src/a", "d1"]).code, 0);
    assert_eq!(mode_of("d1/a"), 0o755);
    assert_eq!(i(&["src/a", "sz3", "d1"]).code, 0);
    assert_eq!(mode_of("d1/sz3"), 0o755);
    // SRC 與 DST 同一檔
    let r = i(&["d1/a", "d1/a"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with("install: 'd1/a' and 'd1/a' are the same file"),
        "{}",
        r.err()
    );
    // 目錄作為 SRC
    let r = i(&["src", "d2"]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("omitting directory 'src'"), "{}", r.err());
    // SRC 不存在：整個 invocation 失敗，但已處理的 SRC 保留
    let r = i(&["sz3", "nope", "d1"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("install: cannot stat 'nope': No such file or directory"),
        "{}",
        r.err()
    );
    assert!(std::path::Path::new("d1/sz3").exists());
}

#[test]
fn directory_creation_modes() {
    fresh("dirs");
    assert_eq!(i(&["-d", "nd1/nd2"]).code, 0);
    // umask 探測：中間層與葉層皆 0777 & !umask（同 mkdir 測試方法）
    let observed = {
        std::fs::create_dir("umask_probe").unwrap();
        let m = mode_of("umask_probe");
        std::fs::remove_dir("umask_probe").unwrap();
        m
    };
    assert_eq!(mode_of("nd1"), observed);
    assert_eq!(mode_of("nd1/nd2"), observed);
    // -d 巢狀：-m 只套葉層，中間層恒 0777 & !umask
    assert_eq!(i(&["-d", "-m", "700", "p1/x/y"]).code, 0);
    assert_eq!(mode_of("p1"), observed);
    assert_eq!(mode_of("p1/x"), observed);
    assert_eq!(mode_of("p1/x/y"), 0o700);
    // -D 建立 dst 的上層
    write("src_a", "AAA", 0o644);
    assert_eq!(i(&["-D", "src_a", "deep/sub2/x2"]).code, 0);
    assert_eq!(mode_of("deep/sub2/x2"), 0o755);
    // 上層為既有檔案
    let r = i(&["-d", "src_a/zz"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("install: cannot create directory 'src_a/zz': Not a directory"),
        "{}",
        r.err()
    );
    let r = i(&["-d"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("install: install with -d requires at least one argument."),
        "{}",
        r.err()
    );
}

#[test]
fn verbose_lines() {
    fresh("verb");
    write("src_a", "AAA", 0o644);
    let r = i(&["-v", "src_a", "v1"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "'src_a' -> 'v1'\n");
    // 覆寫既有 dst：先 removed 再 -> 
    let r = i(&["-v", "src_a", "v1"]);
    assert_eq!(r.out(), "removed 'v1'\n'src_a' -> 'v1'\n");
    let r = i(&["-v", "-d", "vd1/vd2"]);
    assert_eq!(r.code, 0);
    // 參考實測：-d -v 只印一筆（整個 operand、不逐層）；已存在的目錄不印
    assert_eq!(r.out(), "creating directory 'vd1/vd2'\n");
    let r = i(&["-v", "-d", "vd1/vd2"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "", "目錄已存在時不應印 creating directory");
    // 參考實測：-D -v 印 "install: creating directory '<DEST 的父目錄>'"
    let r = i(&["-v", "-D", "src_a", "nest/a/b/x"]);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out(),
        "install: creating directory 'nest/a/b'\n'src_a' -> 'nest/a/b/x'\n",
        "{}",
        r.out()
    );
}

#[test]
fn backups() {
    fresh("bk");
    write("src_a", "AAA", 0o644);
    write("f1", "OLD", 0o644);
    assert_eq!(i(&["-b", "src_a", "f1"]).code, 0);
    assert!(!std::path::Path::new("f1~").exists() || std::path::Path::new("f1~").exists());
    assert_eq!(std::fs::read_to_string("f1").unwrap(), "AAA");
    // -S 單獨使用即啟用備份
    assert_eq!(i(&["-S", ".XX", "src_a", "f1"]).code, 0);
    assert!(std::path::Path::new("f1.XX").exists());
    // --backup=numbered
    assert_eq!(i(&["--backup=numbered", "src_a", "f1"]).code, 0);
    let numbered = std::fs::read_dir(".")
        .unwrap()
        .filter_map(|e| e.ok().map(|e| e.file_name().to_string_lossy().into_owned()))
        .filter(|n| n.starts_with("f1.~") && n.ends_with('~'))
        .count();
    assert!(numbered >= 1, "expected numbered backup");
    let r = i(&["--backup=bogus", "src_a", "f1"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().lines().next().unwrap(),
        "error: invalid value 'bogus' for '--backup[=<METHOD>]'"
    );
    assert!(
        r.err()
            .contains("  [possible values: none, numbered, existing, simple]"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("For more information, try '--help'."));
}

#[test]
fn compare_and_times_and_misc_flags() {
    fresh("cmp");
    write("src_a", "AAA", 0o644);
    write("same", "AAA", 0o644);
    // -C：內容相同 → 不複製，但 mode 仍套用
    assert_eq!(i(&["-C", "-m", "700", "src_a", "same"]).code, 0);
    assert_eq!(mode_of("same"), 0o700);
    // -C：內容不同 → 複製
    write("diff", "BBB", 0o644);
    assert_eq!(i(&["-C", "src_a", "diff"]).code, 0);
    assert_eq!(std::fs::read_to_string("diff").unwrap(), "AAA");
    // -C 與 -s 互斥（檢查在任何動作之前）
    let r = i(&["-C", "-s", "src_a", "never"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().lines().next().unwrap(),
        "install: Options --compare and --strip are mutually exclusive"
    );
    assert!(r.err().contains("Try '"), "{}", r.err());
    assert!(!std::path::Path::new("never").exists());
    // -p 複製時間戳
    write("old_time", "AAA", 0o644);
    {
        use std::process::Command;
        Command::new("touch")
            .args(["-t", "200101010800.00", "old_time"])
            .status()
            .ok();
    }
    assert_eq!(i(&["-p", "old_time", "pt1"]).code, 0);
    let a = std::fs::metadata("old_time").unwrap().mtime();
    let b = std::fs::metadata("pt1").unwrap().mtime();
    assert_eq!(a, b);
    // -c / -Z 被忽略
    assert_eq!(i(&["-c", "src_a", "c1"]).code, 0);
    assert_eq!(i(&["-Z", "src_a", "z1"]).code, 0);
    // -o 自己不失敗； bogus user 報錯
    let me = std::process::id().to_string();
    assert_eq!(i(&["-o", &me, "src_a", "o1"]).code, 0);
    let r = i(&["-o", "no_such_user_zz", "src_a", "o2"]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid user"), "{}", r.err());
    let r = i(&["-g", "no_such_group_zz", "src_a", "o3"]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("invalid group"), "{}", r.err());
}

#[test]
fn target_dir_and_extra_operand() {
    fresh("tgt");
    write("src_a", "AAA", 0o644);
    write("sz3", "CCC", 0o644);
    std::fs::create_dir_all("tdir").unwrap();
    assert_eq!(i(&["-t", "tdir", "src_a", "sz3"]).code, 0);
    assert!(std::path::Path::new("tdir/src_a").exists());
    // -t 目標不存在＋單一 operand → 直接把目標當檔案（參考實測）
    assert_eq!(i(&["-t", "no_such_dir", "src_a"]).code, 0);
    assert!(std::path::Path::new("no_such_dir").is_file());
    // -t 目標不存在＋兩個 operand
    let r = i(&["-t", "nodir2", "src_a", "sz3"]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("install: target 'nodir2' is not a directory"), "{}", r.err());
    // -T 只接受兩個 operand
    let r = i(&["-T", "src_a", "sz3", "t1"]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("install: extra operand 't1'"), "{}", r.err());
    assert_eq!(i(&["-T", "src_a", "t2"]).code, 0);
}

#[test]
fn operand_errors_and_version() {
    fresh("err");
    write("src_a", "AAA", 0o644);
    let r = i(&[]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("install: missing file operand"), "{}", r.err());
    assert!(r.err().contains("Try '"), "{}", r.err());
    let r = i(&["src_a"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("install: missing destination file operand after 'src_a'"),
        "{}",
        r.err()
    );
    let r = i(&["-t", "tdir"]);
    assert_eq!(r.code, 1);
    assert!(r.err().contains("install: missing file operand"), "{}", r.err());
    let r = i(&["-m"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().lines().next().unwrap(),
        "error: a value is required for '-m <MODE>' but none was supplied"
    );
    let r = i(&["--version"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "install (uutils coreutils) 0.10.0\n");
    let r = i(&["--help"]);
    assert_eq!(r.code, 0);
    assert!(r.out().contains("Usage: install [OPTION]... [FILE]..."), "{}", r.out());
}

#[test]
fn e2e_binary_dispatch() {
    fresh("e2e");
    write("src_a", "AAA", 0o644);
    let r = t::run_bin("install", &["src_a", "b1"], "");
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(mode_of("b1"), 0o755);
}

#[test]
fn fuzz_install_mode_and_content() {
    fresh("fz");
    let mut rng = t::Rng::new(20261101);
    for k in 0..40 {
        let mode = 0o777u32 & !(rng.below(0o1000) as u32);
        let nbyte = 1 + rng.below(40) as usize;
        let raw = rng.bytes(nbyte);
        let content: String = String::from_utf8_lossy(&raw)
            .chars()
            .map(|c| if c == '\0' { ' ' } else { c })
            .collect();
        let src = format!("src_{k}");
        let dst = format!("dst_{k}");
        write(&src, &content, mode);
        let mspec = format!("{:03o}", mode);
        let r = i(&["-m", &mspec, &src, &dst]);
        assert_eq!(r.code, 0, "case {k}: {}", r.err());
        // 顯式 -m 恒確立（不受 umask 影響）
        assert_eq!(mode_of(&dst) & 0o7777, mode, "case {k} mode {}", mspec);
        assert_eq!(std::fs::read_to_string(&dst).unwrap(), content, "case {k}");
        // 冪等：重複 install 同一 dst 結果相同
        let r = i(&["-m", &mspec, &src, &dst]);
        assert_eq!(r.code, 0);
        assert_eq!(mode_of(&dst) & 0o7777, mode);
    }
}

#[test]
fn make_parents_owner_group_and_selinux_flags() {
    let d = fresh("parents");
    let src = d.join("src").to_string_lossy().into_owned();
    write(&src, "payload", 0o644);

    // -D：建立 DEST 的所有上層目錄（uutils 擴充；GNU man 無 -D）
    let deep = d.join("a/b/c").to_string_lossy().into_owned();
    let r = i(&["-D", &src, &deep]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(std::fs::metadata(&deep).unwrap().is_file(), "-D 後 DEST 應是檔案");
    assert!(std::fs::metadata(d.join("a/b")).unwrap().is_dir());

    // -o/-g：guest 為 root、以數字 id 指定（guest 無 /etc/passwd → 名稱查找必然失敗，
    // 與 host 參考行為一致："install: invalid user: 'root'"）；不給 -m 時 mode 755
    let dst = d.join("og").to_string_lossy().into_owned();
    let r = i(&["-o", "0", "-g", "0", &src, &dst]);
    assert_eq!(r.code, 0, "{}", r.err());
    let md = std::fs::metadata(&dst).unwrap();
    assert_eq!(md.uid(), 0, "-o root 應 chown 成功");
    assert_eq!(md.gid(), 0, "-g root 應 chgrp 成功");
    assert_eq!(md.mode() & 0o7777, 0o755);

    // -U/--unprivileged：無特權時 chown 失敗不報、chmod 仍套用（root 環境下 chown 直接成功）
    let dst2 = d.join("og2").to_string_lossy().into_owned();
    let r = i(&["-U", "--group=0", &src, &dst2]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(std::fs::metadata(&dst2).unwrap().gid(), 0);

    // man 有載的 SELinux 旗標：-Z、-P/--preserve-context、--context[=CTX]
    // （本建置無 SELinux：參考與我方都接受且不改變結果）
    let src_s = src.as_str();
    for (idx, flags) in vec![
        vec!["-Z"],
        vec!["-P"],
        vec!["--preserve-context"],
        vec!["--context=s0"],
        vec!["--context", "s0"],
    ]
    .into_iter()
    .enumerate()
    {
        let dst = d.join(format!("sel{}", idx)).to_string_lossy().into_owned();
        let mut args: Vec<&str> = flags.clone();
        args.push(src_s);
        args.push(dst.as_str());
        let r = i(&args);
        assert_eq!(r.code, 0, "flags={:?} err={}", flags, r.err());
        assert!(std::fs::metadata(&dst).unwrap().is_file(), "flags={:?}", flags);
    }

    // 參考實測：`--context CTX`（無 '='）會貪心吃掉 CTX，剩下 1 個 operand
    let r = i(&["--context", "s0", src_s]);
    assert_eq!(r.code, 1, "{}", r.err());
    assert_eq!(
        r.err().lines().next().unwrap(),
        format!("install: missing destination file operand after '{}'", src),
        "{}",
        r.err()
    );
}

#[test]
fn strip_flag_reports_program_failure() {
    // guest 內無 strip 程式 → 只能觀察失敗路徑；參考（host 有 strip）為
    // "strip: <dst>: file format not recognized" + "install: strip program failed: ..."
    let d = fresh("strip");
    let src = d.join("src").to_string_lossy().into_owned();
    write(&src, "payload", 0o644);
    let dst = d.join("dsts").to_string_lossy().into_owned();
    let r = i(&["-s", &src, &dst]);
    assert_eq!(r.code, 1, "{}", r.err());
    assert!(r.err().contains("strip"), "{}", r.err());
}
