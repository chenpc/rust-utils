//! env(1) guest e2e 測試。
//!
//! 斷言依據：docs/man/env.1.gz（OPTIONS 清單、`-` 等同 `-i`、`-S` 只切分後續 args、
//! `-a` 在 `-S` 之後可用、`-f` 的 ".env" 設定「prior to any unset and/or set」）
//! ＋ man 未載明時由本機參考實測決定的部分（exit code 分級 125/126/127、clap 措辭、
//! `--list-signal-handling` 的 `{:<11}({}): {}` 格式），見 docs/assumptions.md。
//!
//! 重要：本專案的 env 用 `std::process::Command` 執行 COMMAND（不是 exec 取代行程；
//! `t::run` 是 in-process，用 exec 會取代 test binary）。因此「命令輸出」的斷言一律
//! 走 `t::run_bin`（真正啟動 multi-call binary），純選項驗證可用 `t::run`。
//! 命令本身用「把 multi-call binary 複製成 <dir>/true、<dir>/echo …」提供：src/main.rs
//! 依 argv[0] 的 basename 分派 applet，所以這些複製檔就是可用的外部命令。

use coreutils::testutil as t;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn fresh(tag: &str) -> PathBuf {
    let dir = t::workdir().join(format!("env_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create_dir_all");
    dir
}

fn host_binary() -> PathBuf {
    PathBuf::from(
        std::env::var("COREUTILS_BIN")
            .unwrap_or_else(|_| "/mnt/bin/coreutils".to_string()),
    )
}

/// 把 multi-call binary symlink 成 <dir>/<name>，回傳路徑。
/// 用 symlink 而非 copy：instrumented debug binary 很大、guest /tmp 是 tmpfs，
/// 複製多次會 StorageFull；argv[0] basename 分派对 symlink 同样成立。
fn make_cmd(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    std::os::unix::fs::symlink(host_binary(), &p).expect("symlink binary");
    p
}

fn run(args: &[&str]) -> t::Out {
    t::run_bin("env", args, "")
}

#[test]
fn prints_environment_when_no_command() {
    let r = run(&["-i", "A=1", "B=2"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out(), "A=1\nB=2\n");
    // 第一個 NAME=VALUE 之後仍是設定的 token 繼續當設定
    let r = run(&["-i", "A=1", "B=2", "C=3"]);
    assert_eq!(r.out(), "A=1\nB=2\nC=3\n");
    // 重複設定同一名稱：後面的值贏、不新增第二筆
    let r = run(&["-i", "A=1", "A=2"]);
    assert_eq!(r.out(), "A=2\n");
    // 單一 `-` 等同 -i（man EXTRA）
    let r = run(&["-", "A=1"]);
    assert_eq!(r.out(), "A=1\n");
    // 沒有一般環境時（不給 -i）仍會印繼承來的環境
    let r = run(&["ENVTEST_MARKER=1"]);
    assert!(r.out().contains("ENVTEST_MARKER=1\n"), "{}", r.out());
    assert!(r.out().lines().all(|l| l.contains('=')), "{}", r.out());
}

#[test]
fn null_flag_separates_pairs_with_nul() {
    let r = run(&["-i", "-0", "A=1", "B=2"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out_bytes(), b"A=1\x00B=2\x00");
    let r = run(&["-i", "--null", "A=1"]);
    assert_eq!(r.out_bytes(), b"A=1\x00");
}

#[test]
fn unset_order_and_precedence() {
    // -u 在設定之後 → 被移除
    let r = run(&["-i", "A=1", "-u", "A"]);
    assert_eq!(r.out(), "");
    // -u 在設定之前 → 保留
    let r = run(&["-i", "-u", "A", "A=1"]);
    assert_eq!(r.out(), "A=1\n");
    // 移除既有環境變數
    let r = run(&["-u", "PATH", "ENVTEST_KEEP=1"]);
    assert_eq!(r.code, 0);
    assert!(!r.out().contains("PATH="), "{}", r.out());
    assert!(r.out().contains("ENVTEST_KEEP=1"));
}

#[test]
fn file_option_reads_dotenv_style() {
    let dir = fresh("file");
    let f = dir.join("cfg.env");
    // 本專案採用的 ".env" 文法（man 只寫 read and set variables from a ".env"-style
    // configuration file；細部規則自行決定，見 docs/assumptions.md）：
    // 略過空白行與 # 註解、以第一個 '=' 切分、兩側 trim、去掉成對引號、不展開 $。
    std::fs::write(
        &f,
        "A=1\n\n# comment\n B2 = spaced \nC=\"q w\"\nD='s d'\nE=$A\nF=x=y\nG=\nnoequals\nH=\"a\"b\n",
    )
    .expect("write");
    let fs = f.to_str().unwrap();
    let r = run(&["-i", "-f", fs]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(
        r.out(),
        "A=1\nB2=spaced\nC=q w\nD=s d\nE=$A\nF=x=y\nG=\nH=ab\n"
    );
    // man: 檔內設定一律先於任何 unset
    let r = run(&["-i", "-f", fs, "-u", "A"]);
    assert_eq!(r.code, 0);
    assert!(!r.out().contains("A=1"), "{}", r.out());
    // 多個 -f 依序套用
    let f2 = dir.join("cfg2.env");
    std::fs::write(&f2, "A=second\nZ=9\n").expect("write");
    let r = run(&["-i", "-f", fs, "-f", f2.to_str().unwrap()]);
    assert_eq!(r.code, 0);
    assert!(r.out().contains("A=second\n"), "{}", r.out());
    assert!(r.out().contains("Z=9\n"));
    // 檔不存在 → rc1（不是 125）
    let r = run(&["-f", "/nonexistent-env-file.env"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err()
            .starts_with("env: /nonexistent-env-file.env: No such file or directory (os error 2)"),
        "{}",
        r.err()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn signal_handling_options() {
    let r = run(&["--ignore-signal=INT", "--list-signal-handling", "-i", "A=1"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.err(), "INT        (2): IGNORE\n");
    assert_eq!(r.out(), "A=1\n");

    // 多個信號逗號分隔；清單依信號編號遞升排序（實測參考行為）
    let r = run(&[
        "--ignore-signal=TERM,INT",
        "--block-signal=1",
        "--default-signal=15",
        "--list-signal-handling",
        "-i",
    ]);
    assert_eq!(r.code, 0, "{}", r.err());
    // 同一信號只保留第一次出現的 action（實測 --ignore-signal=INT --default-signal=INT → IGNORE）
    assert_eq!(
        r.err(),
        "HUP        (1): BLOCK\nINT        (2): IGNORE\nTERM       (15): IGNORE\n"
    );

    // 同一信號重複指定：只保留第一次的 action
    let r = run(&[
        "--ignore-signal=INT",
        "--default-signal=INT",
        "--list-signal-handling",
        "-i",
    ]);
    assert_eq!(r.err(), "INT        (2): IGNORE\n");

    // 名稱可含 SIG 前綴、可大小寫混合、可用純數字
    for spec in ["SIGINT", "int", "2", "SIGTERM"] {
        let flag = format!("--ignore-signal={}", spec);
        let r = run(&[&flag, "--list-signal-handling", "-i"]);
        assert_eq!(r.code, 0, "{} → {}", spec, r.err());
        assert!(r.err().contains(": IGNORE"), "{} → {}", spec, r.err());
    }
    // 裸 --ignore-signal（無值）不記任何信號
    let r = run(&["--ignore-signal", "--list-signal-handling", "-i"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.err(), "");
    // --block-signal=9 合法（BLOCK 允许 KILL）
    let r = run(&["--block-signal=9", "--list-signal-handling", "-i"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.err(), "KILL       (9): BLOCK\n");
    let _ = std::fs::remove_dir_all(fresh("sigclean"));
}

#[test]
fn signal_errors_are_rc125() {
    for spec in ["all", "bogussig", "sig9", "0", "INT,bogussig"] {
        let flag = format!("--ignore-signal={}", spec);
        let r = run(&[&flag, "-i"]);
        assert_eq!(r.code, 125, "{}", spec);
        let want = if spec.contains(',') {
            "env: 'bogussig': invalid signal\n".to_string()
        } else {
            format!("env: '{}': invalid signal\n", spec)
        };
        assert_eq!(r.err(), want, "{}", spec);
    }
    // 不可更改 SIGKILL 的 action
    for spec in ["9", "SIGKILL"] {
        let flag = format!("--ignore-signal={}", spec);
        let r = run(&[&flag, "-i"]);
        assert_eq!(r.code, 125, "{}", spec);
        assert_eq!(
            r.err(),
            "env: failed to set signal action for signal 9: Invalid argument\n",
            "{}",
            spec
        );
    }
}

#[test]
fn option_conflicts_are_rc125() {
    let r = run(&["-0", "/bin/sh"]);
    assert_eq!(r.code, 125);
    assert!(
        r.err()
            .starts_with("env: cannot specify --null (-0) with command"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("Try '"), "{}", r.err());

    // -C 沒有 command
    for dir in ["/tmp", "/nonexistent-dir-env"] {
        let r = run(&["-C", dir]);
        assert_eq!(r.code, 125, "{}", dir);
        assert!(
            r.err()
                .starts_with("env: must specify command with --chdir (-C)"),
            "{} → {}",
            dir,
            r.err()
        );
    }
    // 兩者同時存在時，先檢查 chdir（因為沒有 command，-0 的檢查不觸發）
    let r = run(&["-0", "-C", "/tmp"]);
    assert_eq!(r.code, 125);
    assert!(
        r.err()
            .starts_with("env: must specify command with --chdir (-C)"),
        "{}",
        r.err()
    );
}

#[test]
fn chdir_is_applied_to_the_child() {
    let dir = fresh("chdir");
    let pwd_cmd = make_cmd(&dir, "pwd");
    let ds = dir.to_str().unwrap();
    // child 的 cwd 應是 -C 指定的目錄（pwd 印 physical cwd）
    let r = run(&["-C", ds, pwd_cmd.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out().trim_end(), ds);
    // 沒有 -C 時 child 继承 parent 的 cwd
    let r2 = run(&[pwd_cmd.to_str().unwrap()]);
    assert_eq!(r2.code, 0);
    let parent = std::env::current_dir().unwrap();
    assert_eq!(r2.out().trim_end(), parent.to_str().unwrap().to_string());
    // chdir 失敗（不存在 / 不是目錄）→ 125，措辭含 os error
    let r3 = run(&["-C", "/nonexistent-env-dir", pwd_cmd.to_str().unwrap()]);
    assert_eq!(r3.code, 125);
    assert!(
        r3.err()
            .starts_with("env: cannot change directory to '/nonexistent-env-dir': No such file or directory (os error 2)"),
        "{}",
        r3.err()
    );
    let host_file = dir.join("notadir");
    std::fs::write(&host_file, b"x").expect("write");
    let host_file = host_file.to_str().unwrap();
    let r4 = run(&["-C", host_file, pwd_cmd.to_str().unwrap()]);
    assert_eq!(r4.code, 125);
    assert!(
        r4.err()
            .starts_with(&format!(
                "env: cannot change directory to '{}': Not a directory (os error 20)",
                host_file
            )),
        "{}",
        r4.err()
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn command_lookup_exit_codes() {
    let dir = fresh("lookup");
    let true_cmd = make_cmd(&dir, "true");
    let false_cmd = make_cmd(&dir, "false");
    let echo_cmd = make_cmd(&dir, "echo");
    let ts = true_cmd.to_str().unwrap();
    let fs = false_cmd.to_str().unwrap();
    let es = echo_cmd.to_str().unwrap();

    let r = run(&["-i", ts]);
    assert_eq!(r.code, 0, "{}", r.err());
    let r = run(&["-i", fs]);
    assert_eq!(r.code, 1);
    // 命令的 argv 原樣傳遞（-n 由 echo applet 處理）
    let r = run(&["-i", es, "-n", "hi"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "hi");
    // 命令找不到 → 127 + hint
    let r = run(&["-i", "/definitely-not-a-command-env"]);
    assert_eq!(r.code, 127);
    assert!(
        r.err()
            .starts_with("env: '/definitely-not-a-command-env': No such file or directory"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("env: use -[v]S to pass options in shebang lines"), "{}", r.err());
    // 找到但不可執行（一般檔、目錄）→ 126，不附 hint
    let plain = dir.join("plain.txt");
    std::fs::write(&plain, "x").expect("write");
    std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    let ps = plain.to_str().unwrap();
    let r = run(&["-i", ps]);
    assert_eq!(r.code, 126, "{}", r.err());
    assert!(
        r.err()
            .starts_with(&format!("env: '{}': Permission denied", ps)),
        "{}",
        r.err()
    );
    assert!(!r.err().contains("shebang lines"), "{}", r.err());
    let ds = dir.to_str().unwrap();
    let r = run(&["-i", ds]);
    assert_eq!(r.code, 126);
    // PATH 查詢用的是「修改後」的環境
    let path_s = format!("PATH={}", ds);
    let r = run(&["-i", &path_s, "true"]);
    assert_eq!(r.code, 0, "{}", r.err());
    // PATH 被移除後仍有內建預設 PATH（guest 內 /bin 只有 sh，故 true 找不到 → 127）
    let r = run(&["-i", "-u", "PATH", "true"]);
    assert_eq!(r.code, 127, "{}", r.err());
    // 命令存在於 PATH 但用相對名稱也能找到
    let r = run(&["-i", &path_s, "echo", "x"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "x\n");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn argv0_override() {
    let dir = fresh("argv0");
    // 複製檔的名字不對應任何 applet：argv[0] 決定分派（src/main.rs 用 basename）
    let odd = dir.join("zzznotanapplet");
    std::os::unix::fs::symlink(host_binary(), &odd).expect("symlink");
    let os = odd.to_str().unwrap();
    // 不覆寫 argv0 → unknown applet（rc2 來自被執行的程式本身）
    let r = run(&["-i", os]);
    assert_eq!(r.code, 2, "{} {} | {}", r.code, r.out(), r.err());
    // -a 覆寫 argv0 → 改用 true applet、rc0
    let r = run(&["-i", "-a", "true", os]);
    assert_eq!(r.code, 0, "{}", r.err());
    // --argv0=<v> 同義
    let r = run(&["-i", "--argv0=false", os]);
    assert_eq!(r.code, 1);
    // 重複 --argv0：後面的贏（實測參考不報錯）
    let r = run(&["-i", "--argv0=false", "--argv0=true", os]);
    assert_eq!(r.code, 0, "{}", r.err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn first_non_assignment_token_is_the_command() {
    let dir = fresh("cmd");
    let true_cmd = make_cmd(&dir, "true");
    let ts = true_cmd.to_str().unwrap();
    // -v 之後的 -u 被當成命令（因為前面已有 NAME=VALUE）
    let r = run(&["-i", "-v", "A=1", "-u", "B", ts]);
    assert_eq!(r.code, 127, "{}", r.err());
    assert!(r.err().contains("executing: -u\n"), "{}", r.err());
    assert!(r.err().contains("   arg[0]= '-u'\n"), "{}", r.err());
    assert!(r.err().contains("   arg[1]= 'B'\n"), "{}", r.err());
    assert!(r.err().contains(&format!("   arg[2]= '{}'", ts)), "{}", r.err());
    assert!(
        r.err()
            .contains("env: '-u': No such file or directory"),
        "{}",
        r.err()
    );
    // `--` 之後一律是命令
    let r = run(&["-i", "--", ts]);
    assert_eq!(r.code, 0, "{}", r.err());
    // -v 的 executing 段用原始命令字串，不是 PATH 解析後的絕對路徑
    let path_s = format!("PATH={}", dir.to_str().unwrap());
    let r = run(&["-i", "-v", &path_s, "true"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(r.err().starts_with("executing: true\n"), "{}", r.err());
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn verbose_prints_input_args_only_when_debug_twice() {
    // 一次 -v：只有 exec 時印 executing 段
    let r = run(&["-i", "-v", "A=1"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "A=1\n");
    assert_eq!(r.err(), "");
    // 兩次 -v：連沒有命令的情況也印 input args
    let r = run(&["-i", "-v", "-v", "A=1"]);
    assert_eq!(r.code, 0);
    let err = r.err();
    assert!(err.starts_with("input args:\n"), "{}", err);
    assert!(err.contains("arg[0]: 'env'\n"), "{}", err);
    assert!(err.contains("arg[1]: '-i'\n"), "{}", err);
    assert!(err.contains("arg[2]: '-v'\n"), "{}", err);
    assert!(err.contains("arg[3]: '-v'\n"), "{}", err);
    assert!(err.contains("arg[4]: 'A=1'\n"), "{}", err);
    // 兩次 -v + 命令：额外印 executing 段，且顺序在 chdir 错误之后
    let dir = fresh("verb2");
    let cmd = make_cmd(&dir, "env");
    let r = run(&["-i", "-v", "-v", "A=1", cmd.to_str().unwrap()]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(r.err().starts_with("input args:\n"), "{}", r.err());
    assert!(r.err().contains("executing: "), "{}", r.err());
    // -vv 同義（不報重複錯誤）
    let r = run(&["-i", "-vv", "A=1"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(r.err().starts_with("input args:\n"), "{}", r.err());
}

#[test]
fn split_string_expands_options() {
    // man EXTRA：-S 只切分「之後」的 args；引號會被吃掉
    // 命令用 symlink 的 env（guest PATH 內沒有 env）
    let dir = fresh("split3");
    let self_env = make_cmd(&dir, "env");
    let r = run(&["-S", &format!("-i A=\"x y\" {}", self_env.display())]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(r.out().contains("A=x y\n"), "{}", r.out());
    let r = run(&["-S", &format!("-i A='p q' {}", self_env.display())]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(r.out().contains("A=p q\n"), "{}", r.out());
    // 空字串等同沒有展開
    let r = run(&["-S", "", "-i", "A=1"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out(), "A=1\n");
    // 展開後的重複選項照樣觸發 clap 的重複錯誤
    let r = run(&["-S", "-i", "-S", "-i", "A=1"]);
    assert_eq!(r.code, 125, "{}", r.err());
    assert!(
        r.err()
            .starts_with("error: the argument '--ignore-environment' cannot be used multiple times"),
        "{}",
        r.err()
    );
    // 反斜線跳脫引號
    let r = run(&["-S", &format!("-i A=x\\ y {}", self_env.display())]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert!(r.out().contains("A=x y\n"), "{}", r.out());
}

#[test]
fn duplicate_options_are_clap_errors_rc125() {
    let cases: &[&[&str]] = &[
        &["-i", "-i"],
        &["-", "-i"],
        &["-i", "--ignore-environment"],
        &["-0", "-0"],
        &["-00"],
        &["--null", "--null"],
        &["--list-signal-handling", "--list-signal-handling"],
        &["-C", "/tmp", "-C", "/tmp"],
        &["--chdir=/tmp", "--chdir=/var"],
    ];
    for args in cases {
        let r = run(args);
        assert_eq!(r.code, 125, "{:?}", args);
        let want = if args[0] == "-C" || args[0] == "--chdir=/tmp" {
            "error: the argument '--chdir <DIR>' cannot be used multiple times\n"
        } else if args[0] == "-0" || args[0] == "-00" || args[0] == "--null" {
            "error: the argument '--null' cannot be used multiple times\n"
        } else if args[0] == "--list-signal-handling" {
            "error: the argument '--list-signal-handling' cannot be used multiple times\n"
        } else {
            "error: the argument '--ignore-environment' cannot be used multiple times\n"
        };
        assert!(r.err().starts_with(want), "{:?} → {}", args, r.err());
        assert!(
            r.err().contains("For more information, try '--help'."),
            "{:?} → {}",
            args,
            r.err()
        );
        assert!(
            r.err().contains("env: use -[v]S to pass options in shebang lines"),
            "{:?} → {}",
            args,
            r.err()
        );
        assert_eq!(r.out(), "");
    }
    // list-signal-handling 的重複要另外驗證（上面最後兩筆是 chdir）
    let r = run(&["--list-signal-handling", "--list-signal-handling"]);
    assert_eq!(r.code, 125);
    assert!(
        r.err()
            .starts_with("error: the argument '--list-signal-handling' cannot be used multiple times"),
        "{}",
        r.err()
    );
}

#[test]
fn missing_option_values_are_rc125() {
    let cases: &[(&str, &str)] = &[
        ("--argv0", "argv0 <a>"),
        ("-a", "argv0 <a>"),
        ("--chdir", "chdir <DIR>"),
        ("-C", "chdir <DIR>"),
        ("--unset", "unset <NAME>"),
        ("-u", "unset <NAME>"),
        ("--file", "file <PATH>"),
        ("-f", "file <PATH>"),
        ("--split-string", "split-string <S>"),
        ("-S", "split-string <S>"),
    ];
    for (flag, display) in cases {
        let r = run(&["-i", flag]);
        assert_eq!(r.code, 125, "{}", flag);
        assert!(
            r.err()
                .starts_with(&format!("error: a value is required for '--{}' but none was supplied", display)),
            "{} → {}",
            flag,
            r.err()
        );
        assert!(
            r.err().contains("env: use -[v]S to pass options in shebang lines"),
            "{} → {}",
            flag,
            r.err()
        );
    }
    // --ignore-signal 的空值不是錯誤（不記任何信號）
    let r = run(&["--ignore-signal=", "--list-signal-handling", "-i", "A=1"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.err(), "");
    assert_eq!(r.out(), "A=1\n");
}

#[test]
fn unknown_options_are_clap_style_rc125() {
    for args in [&["--bogus"][..], &["-x"][..], &["-1"][..], &["--nulx"][..], &["-i", "-Q"][..]] {
        let r = run(args);
        assert_eq!(r.code, 125, "{:?}", args);
        let err = r.err();
        let lines: Vec<&str> = err.lines().collect();
        assert!(lines[0].starts_with("error: unexpected argument '"), "{:?}", args);
        assert_eq!(lines[1], "");
        assert!(err.contains("  tip: to pass '"), "{:?}", args);
        assert!(
            err.contains("Usage: env [OPTION]... [-] [NAME=VALUE]... [COMMAND [ARG]...]"),
            "{:?}",
            args
        );
        assert!(err.contains("For more information, try '--help'."), "{:?}", args);
        assert!(err.contains("env: use -[v]S to pass options in shebang lines"), "{:?}", args);
    }
}

#[test]
fn help_and_version_are_verbatim() {
    let r = run(&["--help"]);
    assert_eq!(r.code, 0);
    let out = r.out();
    assert!(
        out.starts_with("Set each NAME to VALUE in the environment and run COMMAND\n"),
        "{}",
        out
    );
    assert!(
        out.contains("Usage: env [OPTION]... [-] [NAME=VALUE]... [COMMAND [ARG]...]"),
        "{}",
        out
    );
    assert!(
        out.contains("  -i, --ignore-environment      start with an empty environment"),
        "{}",
        out
    );
    assert!(
        out.contains("  -S, --split-string <S>        process and split S into separate arguments"),
        "{}",
        out
    );
    assert!(
        out.contains("  -a, --argv0 <a>               Override the zeroth argument passed to the command being executed."),
        "{}",
        out
    );
    assert!(
        out.contains("      --list-signal-handling    list signal handling changes requested by preceding options"),
        "{}",
        out
    );
    assert!(
        out.contains("A mere - implies -i. If no COMMAND, print the resulting environment."),
        "{}",
        out
    );
    assert_eq!(t::run("env", &["--help"], "").out(), out);
    assert_eq!(t::run("env", &["--hel"], "").out(), out);
    let v = run(&["--version"]);
    assert_eq!(v.out(), "env (uutils coreutils) 0.10.0\n");
    assert_eq!(t::run("env", &["--vers"], "").out(), v.out());
    // help 先出現就立刻回 0
    let r = run(&["--help", "-i"]);
    assert_eq!(r.code, 0);
    assert!(r.out().starts_with("Set each NAME to VALUE in the environment and run COMMAND"));
}

#[test]
fn misc_edge_cases() {
    // `-- -i`：`-i` 變成命令 → 127
    let r = run(&["--", "-i"]);
    assert_eq!(r.code, 127);
    assert!(
        r.err()
            .starts_with("env: '-i': No such file or directory"),
        "{}",
        r.err()
    );
    // 沒有 NAME=VALUE、沒有命令、沒有 -i：印繼承的環境且回 0
    let r = run(&[]);
    assert_eq!(r.code, 0);
    assert!(!r.out().is_empty());
    // -a 沒有命令也可成功（只是覆寫 argv0 無從觀察）
    let r = run(&["-i", "-a", "myname"]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out(), "");
    // 值裡含 '='：以第一個 '=' 切分
    let r = run(&["-i", "A=b=c"]);
    assert_eq!(r.out(), "A=b=c\n");
    // 值為空的變數
    let r = run(&["-i", "A="]);
    assert_eq!(r.out(), "A=\n");
    // 名稱含空白仍照原樣輸出
    let r = run(&["-i", "A B=c d"]);
    assert_eq!(r.out(), "A B=c d\n");
}

#[test]
fn fuzz_assignments_and_unsets() {
    let mut rng = t::Rng::new(20261127);
    for round in 0..24 {
        let n = 1 + rng.below(5);
        let mut args: Vec<String> = vec!["-i".to_string()];
        let mut table: Vec<(String, String)> = Vec::new();
        let mut saw_assign = false;
        for k in 0..n {
            let name = format!("FV{}", k);
            match rng.below(4) {
                0 | 1 => {
                    let value: String = (0..4)
                        .map(|_| (b'a' + rng.below(26) as u8) as char)
                        .collect();
                    args.push(format!("{}={}", name, value));
                    saw_assign = true;
                    let mut replaced = false;
                    for entry in table.iter_mut() {
                        if entry.0 == name {
                            entry.1 = value.clone();
                            replaced = true;
                            break;
                        }
                    }
                    if !replaced {
                        table.push((name.clone(), value.clone()));
                    }
                }
                2 if !saw_assign => {
                    args.push("-u".to_string());
                    args.push(name.clone());
                    table.retain(|e| e.0 != name);
                }
                _ => {
                    // 空值
                    args.push(format!("{}=", name));
                    saw_assign = true;
                    let mut replaced = false;
                    for entry in table.iter_mut() {
                        if entry.0 == name {
                            entry.1.clear();
                            replaced = true;
                            break;
                        }
                    }
                    if !replaced {
                        table.push((name.clone(), String::new()));
                    }
                }
            }
        }
        let refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
        let r = run(&refs);
        assert_eq!(r.code, 0, "round {}: {}", round, r.err());
        let want: String = table
            .iter()
            .map(|(k, v)| format!("{}={}\n", k, v))
            .collect();
        assert_eq!(r.out(), want, "round {}: {:?}", round, args);
    }
}
