//! stdbuf(1)：`stdbuf [OPTION]... COMMAND`。
//!
//! 對齊基準：本機參考 `/usr/lib/cargo/bin/coreutils/stdbuf`（uutils coreutils 0.10.0）。
//! 80 例 pair-diff 只剩 2 例差異（clap 的 fuzzy long tip，記錄於 docs/assumptions.md）。
//!
//! e2e 一律用「名叫 `stdbuf` 的 symlink → multi-call binary」：argv[0] 的 basename
//! 决定 applet，這樣 `Try '<argv0 目錄>/stdbuf --help'` 這類提示行在 host 與 guest
//! 都長同一個形狀（測試把該路徑正規化成 `<BIN>` 後比對）。
//! `t::run_bin()` 只適合 guest（它讀 COREUTILS_BIN）；host 沒有 per-applet binary，
//! 直接呼叫 `coreutils stdbuf ...` 會拿到 multi-call banner，所以這裡不用它跑 stdbuf。

use coreutils::testutil as t;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

fn host_binary() -> PathBuf {
    if let Ok(p) = std::env::var("COREUTILS_BIN") {
        return PathBuf::from(p);
    }
    let local = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("target/debug/coreutils");
    if local.exists() {
        return local;
    }
    PathBuf::from("/mnt/bin/coreutils")
}

fn fresh(tag: &str) -> PathBuf {
    let dir = t::workdir().join(format!("stdbuf_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("create_dir_all");
    dir
}

fn make_cmd(dir: &Path, name: &str) -> PathBuf {
    let p = dir.join(name);
    let _ = std::fs::remove_file(&p);
    std::os::unix::fs::symlink(host_binary(), &p).expect("symlink binary");
    p
}

fn exe(dir: &Path) -> PathBuf {
    make_cmd(dir, "stdbuf")
}

/// 直接 spawn per-applet 名稱。usage hint 裡的 path 取自 argv[0] 的目錄
/// （host `./target/debug/stdbuf`、guest `/mnt/bin/stdbuf`），一律正規化成 `<BIN>`。
const HINT_TAIL: &str = "--help' for more information.";

fn normalize_hint(text: &str) -> String {
    let mut out = String::new();
    let mut rest = text;
    while let Some(i) = rest.find("Try '") {
        let after = &rest[i + "Try '".len()..];
        let Some(j) = after.find(HINT_TAIL) else { break };
        out.push_str(&rest[..i]);
        out.push_str("Try '<BIN> ");
        out.push_str(HINT_TAIL);
        rest = &after[j + HINT_TAIL.len()..];
    }
    out.push_str(rest);
    out
}

fn spawn(path: &Path, args: &[&str]) -> (i32, String, String) {
    let out = std::process::Command::new(path)
        .args(args)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .expect("run");
    (
        out.status.code().unwrap_or(-1),
        String::from_utf8_lossy(&out.stdout).to_string(),
        normalize_hint(&String::from_utf8_lossy(&out.stderr)),
    )
}

fn run(dir: &Path, args: &[&str]) -> (i32, String, String) {
    spawn(&exe(dir), args)
}

const USAGE: &str = "Usage: stdbuf [OPTION]... COMMAND";
const TIP: &str = "For more information, try '--help'.\n";

fn missing(items: &[&str]) -> String {
    let mut s = String::from("error: the following required arguments were not provided:\n");
    for item in items {
        s.push_str("  ");
        s.push_str(item);
        s.push('\n');
    }
    s.push_str(&format!("\n{USAGE}\n\n{TIP}"));
    s
}

fn value_required(long: &str) -> String {
    format!("error: a value is required for '--{long} <MODE>' but none was supplied\n\n{TIP}")
}

fn unexpected(arg: &str) -> String {
    format!(
        "error: unexpected argument '{arg}' found\n\n  tip: to pass '{arg}' as a value, use '-- {arg}'\n\n{USAGE}\n\n{TIP}"
    )
}

fn dup(long: &str) -> String {
    format!("error: the argument '--{long} <MODE>' cannot be used multiple times\n\n{TIP}")
}

/// 拒收 MODE 的措辭：REF 只在「壞在後綴結構」時加單引號；壞在數字本身（`09` → `9`、
/// `09k`、`00k`、`0xG`、`0B101`）都不加引號。逐條取自本機參考實測。
const BAD_MODES: &[(&str, &str)] = &[
    ("", "''"),
    ("L0", "'L0'"),
    ("1kb", "'1kb'"),
    ("1mb", "'1mb'"),
    ("1Kb", "'1Kb'"),
    ("0kk", "'0kk'"),
    ("0b2", "'0b2'"),
    ("1.5", "'1.5'"),
    (" 1", "' 1'"),
    ("1 ", "'1 '"),
    ("00k", "00k"),
    ("000k", "000k"),
    ("00z", "00z"),
    ("09", "9"),
    ("008", "8"),
    ("09k", "09k"),
    ("0X10", "'0X10'"),
    ("0X", "'0X'"),
    ("0B101", "'0B101'"),
    ("0xG", "0xG"),
];

const TOO_LARGE_MODES: &[(&str, &str)] = &[
    ("17E", "'17E'"),
    ("16E", "'16E'"),
    ("1Y", "'1Y'"),
    ("1y", "'1y'"),
    ("1Z", "'1Z'"),
    ("123456789012345678901234", "'123456789012345678901234'"),
];

fn bad_mode(shown: &str) -> String {
    format!("stdbuf: invalid mode {shown}\nTry '<BIN> --help' for more information.\n")
}

fn too_large(shown: &str) -> String {
    format!(
        "stdbuf: invalid mode {shown}: Value too large for defined data type\nTry '<BIN> --help' for more information.\n"
    )
}

#[test]
fn help_and_version() {
    let dir = fresh("help");
    let (rc, out, err) = run(&dir, &["--help"]);
    assert_eq!((rc, err.as_str()), (0, ""));
    assert!(out.starts_with("Run COMMAND, with modified buffering operations"), "{out}");
    assert!(out.contains("  -i, --input <MODE>   adjust standard input stream buffering"));
    assert!(out.contains("KB 1000, K 1024, MB 1000*1000, M 1024*1024, and so on for G, T, P, E, Z, Y."));
    assert!(out.ends_with("stdbuf settings.\n"), "{out}");

    let (rc, out, err) = run(&dir, &["--version"]);
    assert_eq!((rc, err.as_str()), (0, ""));
    assert_eq!(out, "stdbuf (uutils coreutils) 0.10.0\n");

    // --help/--version 先於一切：後面的 operand 不影響
    for args in [
        vec!["--help", "x"],
        vec!["--version", "x"],
        vec!["--ver", "x"],
        vec!["--hel"],
        vec!["--vers"],
    ] {
        let (rc, _, err) = run(&dir, &args);
        assert_eq!((rc, err.as_str()), (0, ""), "{:?}", args);
    }
    assert_eq!(run(&dir, &["--version", "x"]).1, "stdbuf (uutils coreutils) 0.10.0\n");
    assert!(run(&dir, &["--help", "x"]).1.contains("Usage: stdbuf"));
}

#[test]
fn missing_command_is_a_usage_error() {
    let dir = fresh("missing");
    let (rc, out, err) = run(&dir, &[]);
    assert_eq!(rc, 125);
    assert_eq!(out, "");
    assert_eq!(err, missing(&["--input <MODE>", "--output <MODE>", "--error <MODE>", "<command>..."]));

    for args in [
        vec!["-o", "0"],
        vec!["-i", "0"],
        vec!["-e", "0"],
        vec!["-o", "0", "-i", "0"],
        vec!["--output=0"],
    ] {
        let (rc, out, err) = run(&dir, &args);
        assert_eq!(rc, 125, "{:?}", args);
        assert_eq!(out, "", "{:?}", args);
        assert_eq!(err, missing(&["<command>..."]), "{:?}", args);
    }

    // 有 COMMAND 但沒任何 MODE 旗標：COMMAND 吃掉第一個 positional → 仍要三個 MODE
    let (rc, _, err) = run(&dir, &["sh", "-c", "exit 3"]);
    assert_eq!(rc, 125);
    assert_eq!(err, missing(&["--input <MODE>", "--output <MODE>", "--error <MODE>"]));
}

#[test]
fn accepted_modes_export_the_computed_value() {
    let dir = fresh("mode");
    let child = make_cmd(&dir, "printenv");
    let child = child.to_str().unwrap();
    let cases: [(&str, &str); 12] = [
        ("L", "L"),
        ("0", "0"),
        ("4096", "4096"),
        ("1K", "1024"),
        ("1KB", "1000"),
        ("1k", "1024"),
        ("b", "512"),
        ("0x10", "16"),
        ("0b101", "5"),
        ("010", "8"),
        ("012k", "10240"),
        ("18446744073709551615", "18446744073709551615"),
    ];
    for (mode, value) in cases {
        let (rc, out, err) = run(&dir, &["-o", mode, child, "_STDBUF_O"]);
        assert_eq!((rc, err.as_str()), (0, ""), "mode {mode}");
        assert_eq!(out, format!("{value}\n"), "mode {mode}");
    }
    // 三個緩衝區各自的環境變數
    let (rc, out, err) = run(&dir, &["-i", "0", "-o", "1024", "-e", "4096", child, "_STDBUF_I"]);
    assert_eq!((rc, err.as_str()), (0, ""), "{out} {err}");
    assert_eq!(out, "0\n");
    let (rc, out, _) = run(&dir, &["-i", "0", "-o", "1024", "-e", "4096", child, "_STDBUF_E"]);
    assert_eq!((rc, out.as_str()), (0, "4096\n"));
}

#[test]
fn rejected_modes_exit_125() {
    let dir = fresh("badmode");
    let child = make_cmd(&dir, "printenv");
    let child = child.to_str().unwrap();
    for (mode, shown) in BAD_MODES {
        let (rc, out, err) = run(&dir, &["-o", mode, child]);
        assert_eq!(rc, 125, "mode {mode:?}");
        assert_eq!(out, "", "mode {mode:?}");
        assert_eq!(err, bad_mode(shown), "mode {mode:?}");
    }
    for (mode, shown) in TOO_LARGE_MODES {
        let (rc, _, err) = run(&dir, &["-o", mode, child]);
        assert_eq!(rc, 125, "mode {mode}");
        assert_eq!(err, too_large(shown), "mode {mode}");
    }
    // `-1` 由 clap 攔截（不是 MODE 驗證）
    let (rc, _, err) = run(&dir, &["-o", "-1", child]);
    assert_eq!(rc, 125);
    assert_eq!(err, unexpected("-1"));

    // -i L 是明示拒絕的組合
    let (rc, out, err) = run(&dir, &["-i", "L", child]);
    assert_eq!(rc, 125);
    assert_eq!(out, "");
    assert_eq!(
        err,
        "stdbuf: line buffering stdin is meaningless\nTry '<BIN> --help' for more information.\n"
    );
}

#[test]
fn unknown_arguments_follow_clap() {
    let dir = fresh("clap");
    let (rc, _, err) = run(&dir, &["--bogus"]);
    assert_eq!(rc, 125);
    assert_eq!(err, unexpected("--bogus"));

    let (rc, _, err) = run(&dir, &["-x"]);
    assert_eq!(rc, 125);
    assert_eq!(err, unexpected("-x"));

    // 唯一前綴可解析（guest PATH 沒有 cat，用 symlink 到 multi-call binary，見 log [53]）
    let cat = make_cmd(&dir, "cat");
    let cat = cat.to_str().unwrap();
    let (rc, out, err) = run(&dir, &["--outpu", "L", cat]);
    assert_eq!((rc, out.as_str(), err.as_str()), (0, "", ""));

    // 前綴相符但已附值 → unexpected value
    let (rc, _, err) = run(&dir, &["--output=0", "--"]);
    assert_eq!(rc, 125);
    assert_eq!(err, missing(&["<command>..."]), "{err}");

    // 重複旗標
    for args in [vec!["-o", "L", "-o", "0", "cat"], vec!["--output=L", "--output=0", "cat"]] {
        let (rc, _, err) = run(&dir, &args);
        assert_eq!(rc, 125, "{:?}", args);
        assert_eq!(err, dup("output"), "{:?}", args);
    }
}

#[test]
fn missing_value_is_detected_when_next_token_is_a_flag() {
    let dir = fresh("noval");
    for args in [
        vec!["-o", "-i", "L", "cat"],
        vec!["--output", "--input", "L", "cat"],
        vec!["-o", "--output=L", "cat"],
        vec!["-o", "--", "cat"],
        vec!["-o", "-o", "L", "cat"],
        vec!["-e", "-h"],
        vec!["--input"],
        vec!["-i"],
        vec!["-e"],
    ] {
        let long = match args[0].trim_start_matches('-') {
            "o" | "output" => "output",
            "i" | "input" => "input",
            _ => "error",
        };
        let (rc, out, err) = run(&dir, &args);
        assert_eq!(rc, 125, "{:?}", args);
        assert_eq!(out, "", "{:?}", args);
        assert_eq!(err, value_required(long), "{:?}", args);
    }
}

#[test]
fn attached_invalid_mode_and_short_clusters() {
    let dir = fresh("attach");
    let child = make_cmd(&dir, "printenv");
    let child = child.to_str().unwrap();
    // 附值形式：MODE 驗證的措辭不變
    for args in [vec!["--output=x", child], vec!["-o=", child], vec!["--input=", child]] {
        let (rc, _, err) = run(&dir, &args);
        assert_eq!(rc, 125, "{:?}", args);
        assert!(err.starts_with("stdbuf: invalid mode "), "{:?}", args);
    }
    // 附值/簇形式與分離形式等價
    for (args, value) in [
        (vec!["-oL", child, "_STDBUF_O"], "L\n"),
        (vec!["--output=L", child, "_STDBUF_O"], "L\n"),
        (vec!["-i0", child, "_STDBUF_I"], "0\n"),
        (vec!["-e4096", child, "_STDBUF_E"], "4096\n"),
    ] {
        let (rc, out, err) = run(&dir, &args);
        assert_eq!((rc, err.as_str()), (0, ""), "{:?}", args);
        assert_eq!(out, value, "{:?}", args);
    }
}

#[test]
fn mode_radix_and_unit_variants() {
    let dir = fresh("radix");
    let child = make_cmd(&dir, "printenv");
    let child = child.to_str().unwrap();
    let cases: [(&str, &str); 13] = [
        ("012", "10"),
        ("0777", "511"),
        ("777", "777"),
        ("1KB", "1000"),
        ("1GB", "1000000000"),
        ("1TB", "1000000000000"),
        ("1kB", "1000"),
        ("12kB", "12000"),
        ("K", "1024"),
        ("KB", "1000"),
        ("b", "512"),
        ("01k", "1024"),
        ("010k", "8192"),
    ];
    for (mode, value) in cases {
        let (rc, out, err) = run(&dir, &["-o", mode, child, "_STDBUF_O"]);
        assert_eq!((rc, err.as_str()), (0, ""), "mode {mode}");
        assert_eq!(out, format!("{value}\n"), "mode {mode}");
    }
    // `0b`（沒有二進位數字）等價 0；`0x` 則被拒（見 BAD_MODES）。
    for (mode, value) in [("0b", "0")] {
        let (rc, out, err) = run(&dir, &["-o", mode, child, "_STDBUF_O"]);
        assert_eq!((rc, err.as_str()), (0, ""), "mode {mode}");
        assert_eq!(out, format!("{value}\n"), "mode {mode}");
    }
}


#[test]
fn invalid_radix_and_unit_spellings_are_rejected() {
    let dir = fresh("badradix");
    let child = make_cmd(&dir, "printenv");
    let child = child.to_str().unwrap();
    // 只接受小寫 0x/0b 前綴；`mb` 這種 unit 拼法、八進位裡的 8/9 都拒絕。
    // 措辭（含是否加引號）由 BAD_MODES 逐條比對。
    let only_here = ["0X10", "0xG", "0X", "0B101", "09", "09k", "1mb", "1Kb", "00z"];
    for mode in only_here {
        let (rc, out, err) = run(&dir, &["-o", mode, child]);
        assert_eq!(rc, 125, "mode {mode}");
        assert_eq!(out, "", "mode {mode}");
        let (_, shown) = BAD_MODES
            .iter()
            .find(|(m, _)| *m == mode)
            .unwrap_or_else(|| panic!("{mode} not in BAD_MODES"));
        assert_eq!(err, bad_mode(shown), "mode {mode}");
    }
}

#[test]
fn mode_validation_order_and_early_help() {
    let dir = fresh("order");
    // `-i L` 的檢查先於 MODE 驗證（兩側順序都報 line buffering）
    for args in [
        vec!["-i", "L", "-o", "bogus", "cat"],
        vec!["-o", "bogus", "-i", "L", "cat"],
    ] {
        let (rc, _, err) = run(&dir, &args);
        assert_eq!(rc, 125, "{:?}", args);
        assert!(
            err.starts_with("stdbuf: line buffering stdin is meaningless\n"),
            "{:?} {err}",
            args
        );
    }
    // MODE 驗證依 input → output → error 順序（三個都壞時報 input）
    let (rc, _, err) = run(&dir, &["-o", "1mb", "--input", "09", "--error", "1Z", "cat"]);
    assert_eq!(rc, 125);
    assert!(err.starts_with("stdbuf: invalid mode 9\n"), "{err}");
}

#[test]
fn command_lookup_and_exit_codes() {
    let dir = fresh("lookup");
    let (rc, out, err) = run(&dir, &["-o", "0", "/definitely/not/here"]);
    assert_eq!((rc, out.as_str()), (127, ""));
    assert_eq!(err, "stdbuf: failed to execute process: No such file or directory\n");

    // 存在但不可執行 → 126
    let bad = dir.join("nonexec");
    std::fs::write(&bad, b"#!/bin/sh\nexit 0\n").expect("write");
    std::fs::set_permissions(&bad, std::fs::Permissions::from_mode(0o644)).expect("chmod");
    let (rc, _, err) = run(&dir, &["-o", "0", bad.to_str().unwrap()]);
    assert_eq!(rc, 126);
    assert_eq!(err, "stdbuf: failed to execute process: Permission denied\n");

    // exit code 傳承
    let (rc, out, err) = run(&dir, &["-o", "0", "sh", "-c", "exit 7"]);
    assert_eq!((rc, out.as_str(), err.as_str()), (7, "", ""));
    let (rc, out, _) = run(&dir, &["-o", "0", "sh", "-c", "echo hi"]);
    assert_eq!((rc, out.as_str()), (0, "hi\n"));

    // `--` 之後的 token 是 COMMAND（不是 stdbuf 旗標）
    let (rc, _, err) = run(&dir, &["-o", "0", "--", "-o"]);
    assert_eq!(rc, 127);
    assert_eq!(err, "stdbuf: failed to execute process: No such file or directory\n");
}

#[test]
fn flags_after_command_are_not_stdbuf_flags() {
    let dir = fresh("pos");
    let child = make_cmd(&dir, "printenv");
    let child = child.to_str().unwrap();
    // COMMAND 之後的旗標由 COMMAND 自己處理（printenv 的 clap 錯誤 → 它的 rc）
    let (rc, out, err) = run(&dir, &["-oL", child, "-o"]);
    assert_eq!(rc, 2, "{out} {err}");
    assert!(err.starts_with("error: unexpected argument '-o' found"), "{err}");
    assert!(err.contains("Usage: printenv"), "{err}");

    let (rc, out, _) = run(&dir, &["-oL", child, "--help"]);
    assert_eq!(rc, 0);
    assert!(out.contains("Usage: printenv"), "{out}");
}

#[test]
fn fuzz_mode_validity_invariants() {
    // 不變量：合法 MODE 文法（L/0/數字±unit/0x/0b/八進）→ 執行成功並傳承 exit code；
    // 非法 MODE → 125 且訊息以 "stdbuf: invalid mode" 起頭；缺 COMMAND → clap 錯誤。
    let dir = fresh("fuzz");
    let child = make_cmd(&dir, "true");
    let child = child.to_str().unwrap(); // 名叫 true 的 symlink → multi-call binary（忽略參數、恒 rc 0）
    let valid = ["L", "0", "1k", "0x10", "010", "18446744073709551615", "012k", "1KB", "b", "2MB"];
    let invalid = ["1kb", "0kk", "0b2", "1.5", "+1", "L0", "17E", "1y", "x", "0xg"];
    let mut st: u64 = 0xC0FFEE1;
    for _ in 0..80 {
        st = st.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        let slot = (st >> 40) % 3;
        let which = (st >> 20) % 10;
        let flag = ["-o", "-i", "-e", "--output", "--input", "--error"][(st as usize >> 3) % 6];
        if slot == 0 {
            // -i/--input 的 L 一律被拒（"line buffering stdin is meaningless"，另行記錄）
            let mode = if flag == "-i" || flag == "--input" { "0" } else { valid[which as usize] };
            let (rc, _, err) = run(&dir, &[flag, mode, child, "x"]);
            assert_eq!(rc, 0, "{flag} {mode} err={err}");
        } else if slot == 1 {
            let mode = invalid[which as usize];
            let (rc, _, err) = run(&dir, &[flag, mode, child, "x"]);
            assert_eq!(rc, 125, "{flag} {mode} err={err}");
            assert!(err.starts_with("stdbuf: invalid mode"), "{flag} {mode}: {err}");
        } else {
            // 旗標後直接接 COMMAND（MODE 缺值）→ clap 缺值 125
            let (rc, _, err) = run(&dir, &[flag, child]);
            assert!(rc == 125 || rc == 2, "{flag}: rc={rc} err={err}");
            assert!(err.starts_with("error: "), "{flag}: {err}");
        }
    }
}
