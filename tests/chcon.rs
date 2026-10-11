//! chcon e2e（guest 內執行）
//! 環境行為（已實測記錄）：guest 根檔案系統為 tmpfs，kernel 未掛 SELinux：
//!   - `setfilecon()` 寫 security.* xattr 在 tmpfs 會**成功**（simple_xattr 接受任意 xattr），
//!     所以完整 CONTEXT 形式 rc0（但 label 實際不生效）。
//!   - `getfilecon()` 讀 label 失敗 → partial（-u/-r/-l/-t）與 `--reference` 一律 rc1。
//! 因此本檔驗證：成功路徑的 verbose 措辭、失敗路徑措辭、operand 解析
//! （CONTEXT vs FILE）、遞迴走訪、以及不得改動檔案 mode/size。
//! （host 側因無 tmpfs 权限會 rc1；以下斷言一律以 guest 為準）
use coreutils::testutil as t;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

fn fresh(tag: &str) -> PathBuf {
    let d = PathBuf::from(format!("/tmp/chcon_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn m(args: &[&str]) -> t::Out {
    t::run("chcon", args, "")
}

fn snap(p: &Path) -> (u32, u64) {
    let md = std::fs::metadata(p).unwrap();
    (md.mode() & 0o7777, md.len())
}

#[test]
fn full_context_sets_label_and_reports_with_verbose() {
    let d = fresh("full");
    let f = d.join("f");
    std::fs::write(&f, b"x").unwrap();
    let before = snap(&f);

    // tmpfs 接受 security.* xattr → 成功（label 不生效但呼叫成功）
    let r = m(&["-v", "unconfined_u:object_r:foo_t:s0", &f.to_string_lossy()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out(), "");
    assert_eq!(
        r.err().trim_end(),
        format!("chcon: changing security context of '{}'", f.display()),
        "verbose 措辭依參考逐字"
    );
    assert_eq!(snap(&f), before, "設定 label 不得改動 mode/size");

    // 不存在的檔案 → 存取檢查先失敗（參考：Accessing failed on '<path>'. <reason> (os error N).）
    let missing = d.join("nope").to_string_lossy().into_owned();
    let r = m(&["unconfined_u:object_r:foo_t:s0", &missing]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        format!(
            "chcon: Accessing failed on '{}'. No such file or directory (os error 2).",
            missing
        ),
        "{}",
        r.err()
    );
}

#[test]
fn partial_options_treat_all_positionals_as_files() {
    let d = fresh("partial");
    let f = d.join("f");
    std::fs::write(&f, b"x").unwrap();

    // GNU synopsis：`chcon [OPTION]... [-u USER] [-r ROLE] [-l RANGE] [-t TYPE] FILE...`
    // → 沒有 CONTEXT positional，所有 positional 都是 FILE
    for opt in ["-u", "-r", "-l", "-t"] {
        let r = m(&[opt, "foo", &f.to_string_lossy()]);
        assert_eq!(r.code, 1, "{} {:?}", opt, r.err());
        assert!(
            r.err()
                .contains("Applying partial security context to unlabeled file failed on '"),
            "{} must be treated as FILE: {}",
            opt,
            r.err()
        );
        assert!(!r.err().contains("No files are specified."), "{}: {}", opt, r.err());
    }

    // 多個 FILE 各自一行錯誤
    let g = d.join("g");
    std::fs::write(&g, b"y").unwrap();
    let r = m(&["-t", "foo_t", &f.to_string_lossy(), &g.to_string_lossy()]);
    assert_eq!(r.code, 1);
    let hits = r
        .err()
        .lines()
        .filter(|l| l.contains("Applying partial security context"))
        .count();
    assert_eq!(hits, 2, "{}", r.err());
}

#[test]
fn recursive_walk_reports_every_entry() {
    let d = fresh("rec");
    let root = d.join("r1");
    std::fs::create_dir_all(root.join("r2")).unwrap();
    std::fs::write(root.join("a"), b"a").unwrap();
    std::fs::write(root.join("r2").join("g"), b"g").unwrap();

    let r = m(&["-R", "-t", "foo_t", &root.to_string_lossy()]);
    assert_eq!(r.code, 1);
    let paths: Vec<String> = r
        .err()
        .lines()
        .filter_map(|l| {
            let key = "failed on '";
            l.find(key).map(|i| {
                let rest = &l[i + key.len()..];
                rest.split('\'').next().unwrap_or("").to_string()
            })
        })
        .collect();
    // root + r2 + a + g（遞迴含目錄本身）
    assert_eq!(paths.len(), 4, "{:?}", paths);
    for p in [&root.to_string_lossy().into_owned(), &root.join("r2").to_string_lossy().to_string(), &root.join("a").to_string_lossy().to_string(), &root.join("r2").join("g").to_string_lossy().to_string()] {
        assert!(paths.contains(p), "{:?} 須被走訪", paths);
    }
}

#[test]
fn reference_missing_reports_getfilecon_failure() {
    let d = fresh("ref");
    let f = d.join("f");
    std::fs::write(&f, b"x").unwrap();
    let before = snap(&f);

    let r = m(&["--reference=/nonexistent-rfile", &f.to_string_lossy()]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with(
            "chcon: Getting security context failed. getfilecon() failed on path '/nonexistent-rfile'."
        ),
        "{}",
        r.err()
    );
    assert_eq!(snap(&f), before);

    // RFILE 存在但無 label：仍失敗，但措辭不同（參考 host 實測：沒有 "getfilecon() failed on path"）
    let rfile = d.join("r");
    std::fs::write(&rfile, b"r").unwrap();
    let r = m(&["--reference", &rfile.to_string_lossy(), &f.to_string_lossy()]);
    assert_eq!(r.code, 1);
    assert!(
        r.err()
            .starts_with("chcon: Getting security context failed on '"),
        "{}",
        r.err()
    );
    assert!(
        !r.err().contains("getfilecon() failed on path"),
        "{}",
        r.err()
    );
}

#[test]
fn no_files_specified_and_operand_split() {
    let r = m(&[]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().lines().next().unwrap(), "chcon: No files are specified.");
    assert_eq!(r.err().lines().nth(1), Some(""));
    assert!(r.err().contains("Try '"));

    // 只有 CONTEXT、沒有 FILE
    let r = m(&["some_u:object_r:foo_t:s0"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().lines().next().unwrap(), "chcon: No files are specified.");

    // -v 是選項不是 FILE
    let d = fresh("nofile");
    let f = d.join("f");
    std::fs::write(&f, b"x").unwrap();
    let r = m(&["-v", &f.to_string_lossy()]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().lines().next().unwrap(), "chcon: No files are specified.");
}

#[test]
fn unknown_option_uses_clap_form() {
    let r = m(&["--bogus", "f"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().lines().next().unwrap(), "error: unexpected argument '--bogus' found");
    assert_eq!(r.err().lines().nth(1), Some(""));
    assert_eq!(
        r.err().lines().nth(2).unwrap(),
        "  tip: to pass '--bogus' as a value, use '-- --bogus'"
    );
    assert_eq!(r.err().lines().nth(3), Some(""));
    assert!(
        r.err().contains("Usage: chcon [OPTION]... CONTEXT FILE..."),
        "{}",
        r.err()
    );
    assert!(r.err().contains("For more information, try '--help'."));

    let r = m(&["-q", "f"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().lines().next().unwrap(), "error: unexpected argument '-q' found");
    assert!(r.err().contains("tip: to pass '-q' as a value, use '-- -q'"), "{}", r.err());

    // 本專案未實作 GNU 的 -Z/--context（本機參考亦無）→ 走 unexpected argument
    let r = m(&["-Z", "f"]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().lines().next().unwrap(), "error: unexpected argument '-Z' found");
    let r = m(&["--context=s0", "f"]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().lines().next().unwrap(),
        "error: unexpected argument '--context' found"
    );

    // 長選項前綴推論（clap infer_long_args）：唯一前綴可用（`--referenc` → `--reference`）
    let r = m(&["--referenc=x", "f"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err()
            .starts_with("chcon: Getting security context failed. getfilecon() failed on path 'x'."),
        "{}",
        r.err()
    );
    let r = m(&["--verb", "ctx", "f"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err()
            .starts_with("chcon: Accessing failed on 'f'. No such file or directory"),
        "{}",
        r.err()
    );
}

#[test]
fn preserve_root_and_traverse_flags() {
    // man：--preserve-root（預設）拒絕對 '/' 遞迴操作
    let r = m(&["-R", "unconfined_u:object_r:foo_t:s0", "/"]);
    assert_eq!(r.code, 1, "{}", r.err());
    assert!(
        r.err().contains("It is dangerous to operate recursively on '/'. Use --no-preserve-root to override this failsafe."),
        "{}",
        r.err()
    );
    assert!(
        r.err().contains("chcon: Modifying root path failed on '/'. permission denied."),
        "{}",
        r.err()
    );
    // 非遞迴時不觸發安全網
    let r = m(&["unconfined_u:object_r:foo_t:s0", "/"]);
    assert!(!r.err().contains("dangerous to operate recursively"), "{}", r.err());

    let d = fresh("trv");
    std::fs::create_dir_all(d.join("sub")).unwrap();
    std::fs::write(d.join("sub").join("g"), b"g").unwrap();
    // -H / -L / -P 皆為合法選項（僅在 -R 時有意義）
    for f in ["-H", "-L", "-P"] {
        let r = m(&["-R", f, "-t", "foo_t", &d.to_string_lossy()]);
        assert_eq!(r.code, 1, "{} {:?}", f, r.err());
        assert!(
            !r.err().starts_with("error: unexpected argument"),
            "{} 須為合法選項：{}",
            f,
            r.err()
        );
    }
}

#[test]
fn dereference_default_vs_no_dereference() {
    let d = fresh("derev");
    let link = d.join("l");
    // 懸空 symlink：預設（--dereference）作用於目標 → 失敗；-h 作用於 symlink 本身
    std::os::unix::fs::symlink("/nonexistent-derev-target", &link).unwrap();

    let r = m(&["unconfined_u:object_r:foo_t:s0", &link.to_string_lossy()]);
    assert_eq!(r.code, 1, "預設應跟隨 symlink：{}", r.err());
    assert!(r.err().contains("No such file or directory"), "{}", r.err());

    let r = m(&["-h", "unconfined_u:object_r:foo_t:s0", &link.to_string_lossy()]);
    assert_eq!(r.code, 0, "-h 應直接作用於 symlink：{}", r.err());

    let r = m(&["--dereference", "-h", "unconfined_u:object_r:foo_t:s0", &link.to_string_lossy()]);
    assert_eq!(r.code, 0, "-h 後來的選項應覆寫 --dereference：{}", r.err());

    let r = m(&["--no-dereference", "-h", "unconfined_u:object_r:foo_t:s0", &link.to_string_lossy()]);
    assert_eq!(r.code, 0, "--no-dereference 須為合法長選項：{}", r.err());
}

#[test]
fn help_lists_three_synopses() {
    let r = m(&["--help"]);
    assert_eq!(r.code, 0);
    let out = r.out();
    assert_eq!(
        out.lines().next().unwrap(),
        "Change the SELinux security context of each FILE to CONTEXT."
    );
    assert!(out.contains("Usage: chcon [OPTION]... CONTEXT FILE..."));
    assert!(out.contains("chcon [OPTION]... [-u USER] [-r ROLE] [-l RANGE] [-t TYPE] FILE..."));
    assert!(out.contains("chcon [OPTION]... --reference=RFILE FILE..."));
    for s in [
        "      --dereference",
        "  -h, --no-dereference",
        "      --preserve-root",
        "      --no-preserve-root",
        "  -H",
        "  -L",
        "  -P",
        "  -v, --verbose",
    ] {
        assert!(out.contains(s), "help 須列 {}", s);
    }
    assert!(!out.contains("--context"), "本專案未實作 --context → help 不應列");

    let r = m(&["-V"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out().trim_end(), "chcon (uutils coreutils) 0.10.0");
}

#[test]
fn fuzz_failures_never_mutate_files() {
    let d = fresh("fz");
    let mut rng = t::Rng::new(20261112);
    let ctxs = [
        "unconfined_u:object_r:foo_t:s0",
        "system_u:object_r:home_root_t:s0",
        "foo",
        "a:b",
        "x:y:z:w",
    ];
    let opts = ["-t", "-u", "-r", "-l"];

    for k in 0..20 {
        let name = format!("f{}", k % 6);
        let f = d.join(&name);
        std::fs::write(&f, format!("content{}", k)).unwrap();
        std::fs::set_permissions(&f, std::fs::Permissions::from_mode(0o640)).unwrap();
        let before = snap(&f);

        let use_partial = rng.below(2) == 1;
        let mut args: Vec<String> = Vec::new();
        if use_partial {
            args.push(opts[rng.below(opts.len() as u64) as usize].to_string());
            args.push(format!("t{}", rng.below(100)));
        } else {
            args.push(ctxs[rng.below(ctxs.len() as u64) as usize].to_string());
        }
        let verbose = rng.below(3) == 0;
        if verbose {
            args.push("-v".to_string());
        }
        args.push(f.to_string_lossy().into_owned());

        let r = m(&args.iter().map(|s| s.as_str()).collect::<Vec<_>>());
        if use_partial {
            // partial 需先讀 label（getfilecon）→ 必失败
            assert_eq!(r.code, 1, "case {}: {}", k, r.out());
            assert!(
                r.err().contains("Applying partial security context to unlabeled file failed on '"),
                "case {}: {}",
                k,
                r.err()
            );
        } else {
            assert_eq!(r.code, 0, "case {}: {}", k, r.err());
            if verbose {
                assert_eq!(
                    r.err().trim_end(),
                    format!("chcon: changing security context of '{}'", f.display()),
                    "case {} verbose 措辭",
                    k
                );
            } else {
                assert_eq!(r.err(), "", "case {} 預設不得輸出", k);
            }
        }
        assert_eq!(snap(&f), before, "case {}: 不得改動檔案", k);
    }
}

#[test]
fn traverse_l_and_h_follow_symlinked_dirs() {
    let d = fresh("trav");
    let dir = d.join("dir");
    std::fs::create_dir(&dir).unwrap();
    std::fs::write(dir.join("inner"), b"y").unwrap();
    let link = d.join("link");
    std::os::unix::fs::symlink(&dir, &link).unwrap();
    let ctx = "unconfined_u:object_r:foo_t:s0";
    let link_s = link.to_string_lossy().into_owned();

    // 預設 -P：連字號 operand 不跟進 → 只有它自己
    let r = m(&["-v", "-R", ctx, &link_s]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.err().lines().count(), 1, "{}", r.err());
    assert!(r.err().contains(&link_s), "{}", r.err());

    // -L：跟進連字號目錄（post-order：先 inner 再 link）
    let r = m(&["-v", "-R", "-L", ctx, &link_s]);
    assert_eq!(r.code, 0, "{}", r.err());
    let ls: Vec<String> = r.err().lines().map(str::to_string).collect();
    assert_eq!(ls.len(), 2, "{:?}", ls);
    assert!(ls[0].ends_with("link/inner'"), "{:?}", ls);
    assert!(ls[1].ends_with("link'"), "{:?}", ls);

    // -H：只跟進命令列上的連字號
    let r = m(&["-v", "-R", "-H", ctx, &link_s]);
    assert_eq!(r.code, 0, "{}", r.err());
    let ls: Vec<String> = r.err().lines().map(str::to_string).collect();
    assert_eq!(ls.len(), 2, "{:?}", ls);
    assert!(ls[0].ends_with("inner'"), "{:?}", ls);
    assert!(ls[1].ends_with("link'"), "{:?}", ls);

    // Broken symlink operand → 存取檢查失敗
    let broken = d.join("broken");
    std::os::unix::fs::symlink(d.join("gone"), &broken).unwrap();
    let r = m(&[ctx, &broken.to_string_lossy()]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        format!(
            "chcon: Accessing failed on '{}'. No such file or directory (os error 2).",
            broken.display()
        ),
        "{}",
        r.err()
    );
}

#[test]
fn reference_uses_label_of_existing_file_and_version_flag() {
    let d = fresh("refok");
    let r1 = d.join("r1");
    let tgt = d.join("tgt");
    std::fs::write(&r1, b"x").unwrap();
    std::fs::write(&tgt, b"y").unwrap();
    let ctx = "unconfined_u:object_r:foo_t:s0";

    // 先讓 r1 有 label（tmpfs 接受 security.* xattr）
    let r = m(&[ctx, &r1.to_string_lossy()]);
    assert_eq!(r.code, 0, "{}", r.err());

    // --reference=<有 label 的檔> → 沿用其 label，rc0
    let r = m(&["-v", "--reference", &r1.to_string_lossy(), &tgt.to_string_lossy()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(
        r.err().trim_end(),
        format!("chcon: changing security context of '{}'", tgt.display()),
        "{}",
        r.err()
    );

    let r = m(&["--version"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(
        r.out().starts_with("chcon (uutils coreutils) "),
        "實際：{:?}",
        r.out()
    );
}

#[test]
fn traverse_options_require_recursive_regardless_of_order() {
    let d = fresh("trav");
    let f = d.join("f");
    std::fs::write(&f, b"x").unwrap();
    let sub = d.join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("g"), b"y").unwrap();
    std::os::unix::fs::symlink("sub", d.join("dlink")).unwrap();
    std::os::unix::fs::symlink("f", d.join("link")).unwrap();
    let fs_ = f.to_string_lossy().into_owned();
    let ds = d.to_string_lossy().into_owned();

    // 參考實測：-H/-L/-P 若沒有 -R → clap 群組錯誤（與旗標順序無關）
    for opt in ["-H", "-L", "-P"] {
        let r = m(&[opt, "s0", &fs_]);
        assert_eq!(r.code, 1, "{} {:?}", opt, r.err());
        assert_eq!(
            r.err().lines().next().unwrap(),
            "error: the following required arguments were not provided:",
            "{} {:?}",
            opt,
            r.err()
        );
        assert!(r.err().contains("  --recursive"), "{}", r.err());
        assert!(
            r.err().contains("For more information, try '--help'."),
            "{}",
            r.err()
        );
    }

    // 有 -R 即合法：-R 在前或在後都要可行（舊實作只接受 -R 在前 → 已修）
    for (i, pre) in vec![
        vec!["-L", "-R"],
        vec!["-R", "-L"],
        vec!["-P", "-R"],
        vec!["-R", "-P"],
        vec!["-H", "-R"],
        vec!["-R", "-H"],
    ]
    .into_iter()
    .enumerate()
    {
        let mut a: Vec<&str> = pre.clone();
        a.push("s0");
        a.push(&ds);
        let r = m(&a);
        assert_eq!(r.code, 0, "case {} {:?}: {}", i, pre, r.err());
    }
}

#[test]
fn traversal_counts_match_traverse_semantics() {
    let d = fresh("count");
    let sub = d.join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("g"), b"y").unwrap();
    std::fs::write(d.join("f"), b"x").unwrap();
    std::os::unix::fs::symlink("sub", d.join("dlink")).unwrap();
    std::os::unix::fs::symlink("f", d.join("link")).unwrap();
    let ds = d.to_string_lossy().into_owned();

    // -v 每個處理的檔案印一行；用它數走訪個數
    //   -P：不進入 symlink 目錄 → d, sub, sub/g, dlink, link, f = 6
    //   -L：進入所有 symlink 目錄 → 再加 dlink/g = 7
    //   -H：只進入「command line operand」的 symlink 目錄（operand 是實體目錄）→ 6
    let counted = |flags: &[&str]| -> usize {
        let mut a: Vec<&str> = vec!["-v"];
        a.extend(flags.iter().copied());
        a.push("s0");
        a.push(&ds);
        let r = m(&a);
        assert_eq!(r.code, 0, "{:?}: {}", flags, r.err());
        r.err()
            .lines()
            .filter(|l| l.starts_with("chcon: changing security context of '"))
            .count()
    };
    assert_eq!(counted(&["-R", "-P"]), 6, "-P 不應進入 dlink");
    assert_eq!(counted(&["-R", "-L"]), 7, "-L 應進入 dlink 並處理 dlink/g");
    assert_eq!(counted(&["-R", "-H"]), 6, "-H 只對 command line 上的 symlink 下探");
    assert_eq!(counted(&["-R"]), 6, "預設 traverse 應同 -P");
}

#[test]
fn dereference_recursive_group_and_reference_wording() {
    let d = fresh("deref");
    let f = d.join("f");
    std::fs::write(&f, b"x").unwrap();
    let sub = d.join("sub");
    std::fs::create_dir(&sub).unwrap();
    std::fs::write(sub.join("g"), b"y").unwrap();
    let fs_ = f.to_string_lossy().into_owned();
    let gs = sub.join("g").to_string_lossy().into_owned();
    let ds = d.to_string_lossy().into_owned();

    // 參考實測：--dereference（明講）+ -R 而沒有 -H/-L → 執行期錯誤 + Try 行
    for args in [vec!["-R", "--dereference"], vec!["--dereference", "-R"]] {
        let mut a: Vec<&str> = args.clone();
        a.push("s0");
        a.push(&ds);
        let r = m(&a);
        assert_eq!(r.code, 1, "{:?}: {}", args, r.err());
        assert_eq!(
            r.err().lines().next().unwrap(),
            "chcon: '--recursive' with '--dereference' require either '-H' or '-L'.",
            "{:?}: {}",
            args,
            r.err()
        );
        assert!(r.err().contains("for more information"), "{}", r.err());
    }
    // 補上 -L 或 -H 即可執行
    let r = m(&["--dereference", "-L", "-R", "s0", &ds]);
    assert_eq!(r.code, 0, "{}", r.err());
    // -h（--no-dereference）+ -R 不需要 -H/-L
    let r = m(&["-h", "-R", "s0", &ds]);
    assert_eq!(r.code, 0, "{}", r.err());
    // -H/-L/-P 的長形選項不存在（man/help 沒有列出）
    for opt in ["--no-dereference", "-h"] {
        let r = m(&[opt, "s0", &fs_]);
        assert_eq!(r.code, 0, "{} {:?}", opt, r.err());
    }

    // 參考實測（host）：RFILE 不存在 → 用 getfilecon() 版措辭；
    // RFILE 存在但無 label → "Getting security context failed on 'RFILE'. <reason>."
    let nofile = d.join("no-rfile").to_string_lossy().into_owned();
    let r = m(&["--reference", nofile.as_str(), &gs]);
    assert_eq!(r.code, 1, "{}", r.err());
    assert!(
        r.err().starts_with(&format!(
            "chcon: Getting security context failed. getfilecon() failed on path '{}'.",
            nofile
        )),
        "{}",
        r.err()
    );
    let unlabeled = d.join("unlabeled").to_string_lossy().into_owned();
    std::fs::write(&unlabeled, b"z").unwrap();
    let eq_form = format!("--reference={}", unlabeled);
    let space_form = ["--reference", unlabeled.as_str(), &gs];
    let r = m(&space_form);
    assert_eq!(r.code, 1, "{}", r.err());
    assert!(
        r.err()
            .starts_with("chcon: Getting security context failed on '"),
        "{}",
        r.err()
    );
    assert!(
        !r.err().contains("getfilecon() failed on path"),
        "RFILE 存在時不應使用 getfilecon() 版措辭：{}",
        r.err()
    );
    let r = m(&[eq_form.as_str(), &gs]);
    assert_eq!(r.code, 1, "{}", r.err());
    assert!(
        r.err()
            .starts_with("chcon: Getting security context failed on '"),
        "--reference=<RFILE> 形式措辭：{}",
        r.err()
    );

    // guest 特性：tmpfs 接受 security.* xattr → 先 label 一個檔案，
    // 之後 --reference 就能讀到 label 並成功（cover 成功路徑）
    let r = m(&["unconfined_u:object_r:ref_t:s0", &fs_]);
    assert_eq!(r.code, 0, "{}", r.err());
    let r = m(&["-v", "--reference", &fs_, &gs]);
    assert_eq!(r.code, 0, "--reference 成功路徑：{}", r.err());
    assert!(
        r.err()
            .contains("chcon: changing security context of '"),
        "{}",
        r.err()
    );
}
