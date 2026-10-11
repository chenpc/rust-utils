//! dircolors e2e（guest 內執行）
//! 行為規格：docs/man/dircolors.1.gz（uutils man：選項與 FILE 格式）＋本機實作決定
//! （man 未列 keyword 清單與 TERM filter 細節 → 記錄於 docs/assumptions.md）：
//!   - TERM/COLORTERM 為 glob filter，只影響其**之後**的條目；檔案內沒有 filter 行時一律納入。
//!   - 內建資料庫含 TERM filter 行 → guest（TERM 未設定）預設輸出 `LS_COLORS='';`。
use coreutils::testutil as t;
use std::path::{Path, PathBuf};

fn fresh(tag: &str) -> PathBuf {
    let d = PathBuf::from(format!("/tmp/dircolors_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn m(args: &[&str]) -> t::Out {
    t::run("dircolors", args, "")
}

fn db(d: &Path, name: &str, body: &str) -> String {
    let p = d.join(name);
    std::fs::write(&p, body).unwrap();
    p.to_string_lossy().into_owned()
}

/// 取出 LS_COLORS 條目（sh 與 csh 兩種形式皆處理；保留原始 `k=v` 串）
fn entries(line: &str) -> Vec<String> {
    let start = if let Some(i) = line.find("LS_COLORS='") {
        i + "LS_COLORS='".len()
    } else if let Some(i) = line.find("LS_COLORS '") {
        i + "LS_COLORS '".len()
    } else {
        return Vec::new();
    };
    line[start..]
        .trim_end_matches("';")
        .trim_end_matches('\'')
        .split(':')
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect()
}

fn first_line(o: &t::Out) -> String {
    o.out().lines().next().unwrap_or("").to_string()
}

#[test]
fn default_bourne_form_two_lines() {
    // 內建資料庫有 TERM filter 行：TERM 需媒合才有條目
    std::env::set_var("TERM", "xterm-256color");
    std::env::remove_var("COLORTERM");
    let r = m(&[]);
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    let ls: Vec<&str> = out.lines().collect();
    assert_eq!(ls.len(), 2, "預設輸出應為兩行");
    assert!(ls[0].starts_with("LS_COLORS='") && ls[0].ends_with("';"), "{}", ls[0]);
    assert_eq!(ls[1], "export LS_COLORS");
    let e = entries(ls[0]);
    assert_eq!(e[0], "rs=0");
    assert_eq!(e[1], "di=01;34");
    assert_eq!(e[2], "ln=01;36");
    assert_eq!(e[3], "mh=00");
    assert!(e.len() > 40, "內建資料庫 entry 數量異常: {}", e.len());
    assert!(e.iter().any(|x| x.starts_with("*.")), "應含 extension 條目");
}

#[test]
fn term_filter_semantics_and_unset_default() {
    let d = fresh("filt");
    // filter 行只影響其後的條目
    let a = db(&d, "mix3.txt", "DIR 01;34\nTERM\tq*\n.tar 01;31\n");
    std::env::set_var("TERM", "zz");
    std::env::remove_var("COLORTERM");
    let r = m(&[&a]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(entries(&first_line(&r)), vec!["di=01;34".to_string()]);

    let b = db(&d, "mix4.txt", "TERM\tq*\nDIR 01;34\n.tar 01;31\n");
    let r = m(&[&b]);
    assert_eq!(r.code, 0);
    assert_eq!(entries(&first_line(&r)), Vec::<String>::new());
    assert_eq!(first_line(&r), "LS_COLORS='';");

    let c = db(&d, "mix5.txt", "DIR 01;34\n.tar 01;31\nTERM\tq*\n");
    let r = m(&[&c]);
    assert_eq!(r.code, 0);
    assert_eq!(
        entries(&first_line(&r)),
        vec!["di=01;34".to_string(), "*.tar=01;31".to_string()]
    );

    // filter 媒合時其後條目納入
    let e = db(&d, "mix6.txt", "TERM\tz*\nDIR 01;34\n");
    let r = m(&[&e]);
    assert_eq!(entries(&first_line(&r)), vec!["di=01;34".to_string()]);

    // 沒有 filter 行：TERM 为何都不影響
    let f = db(&d, "nofilter.txt", "DIR 01;34\n.tar 01;31\n");
    std::env::remove_var("TERM");
    let r = m(&[&f]);
    assert_eq!(entries(&first_line(&r)), vec!["di=01;34".to_string(), "*.tar=01;31".to_string()]);

    // 內建資料庫（含 TERM 行）於 TERM 未設定時为空
    let r = m(&[]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(first_line(&r), "LS_COLORS='';");
    std::env::set_var("TERM", "xterm-256color");
}

#[test]
fn csh_form_single_setenv_line() {
    std::env::set_var("TERM", "xterm-256color");
    let r = m(&["-c"]);
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    assert_eq!(out.lines().count(), 1, "-c 應只有一行");
    assert!(out.starts_with("setenv LS_COLORS '") && out.trim_end().ends_with('\''), "{}", out);
    assert!(!out.contains("export"));
    assert_eq!(entries(out.lines().next().unwrap()).len(), entries(&first_line(&m(&[]))).len());

    // -b 與預設相同
    let r = m(&["-b"]);
    assert_eq!(r.out(), m(&[]).out());
}

#[test]
fn print_database_roundtrip() {
    std::env::set_var("TERM", "xterm-256color");
    let p = m(&["-p"]);
    assert_eq!(p.code, 0);
    let dbtext = p.out();
    assert!(dbtext.starts_with("# Configuration file for dircolors, a utility to help you set the"));
    assert!(dbtext.lines().count() >= 100, "-p 資料庫行數異常");
    assert!(dbtext.contains("TERM ?*") || dbtext.contains("TERM "), "-p 應含 TERM 行");

    // 把自家資料庫餵回自己，應得到與預設相同的 LS_COLORS
    let r = t::run_bytes("dircolors", &[], dbtext.as_bytes());
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out(), m(&[]).out(), "-p 資料庫輪替須一致");
}

#[test]
fn custom_keywords_and_extensions_in_file_order() {
    let d = fresh("ord");
    std::env::set_var("TERM", "zz");
    std::env::remove_var("COLORTERM");
    let f = db(
        &d,
        "db1.txt",
        "DIR 01;34\n.tar 01;31\n.core 00;32\nEXEC 01;32\n",
    );
    let r = m(&[&f]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(r.out().lines().nth(1), Some("export LS_COLORS"));
    assert_eq!(
        entries(&first_line(&r)),
        vec![
            "di=01;34".to_string(),
            "*.tar=01;31".to_string(),
            "*.core=00;32".to_string(),
            "ex=01;32".to_string(),
        ],
        "keyword 對應與檔案順序須保留"
    );

    // GNU 別名（本機決定，man 未列清單）
    let g = db(&d, "alias.txt", "NORMAL 00\nFILE 00\nPIPE 40;33\nLEFT [\nRIGHT ]\n");
    let r = m(&[&g]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(
        entries(&first_line(&r)),
        vec![
            "fi=00".to_string(),
            "fi=00".to_string(),
            "pi=40;33".to_string(),
            "lc=[".to_string(),
            "rc=]".to_string(),
        ],
        "GNU keyword 別名"
    );

    // 識別但忽略的 keyword 不產生條目
    let h = db(&d, "ign.txt", "OPTIONS --color\nCOLOR yes\nEIGHTBIT 1\nDIR 01;34\n");
    let r = m(&[&h]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(entries(&first_line(&r)), vec!["di=01;34".to_string()]);

    // 註解與空行略過
    let i = db(&d, "cm.txt", "# comment\n\n   \nDIR 01;34\n");
    let r = m(&[&i]);
    assert_eq!(r.code, 0, "{}", r.err());
    assert_eq!(entries(&first_line(&r)), vec!["di=01;34".to_string()]);
}

#[test]
fn extension_key_containing_equals_is_not_duplicated() {
    let d = fresh("eq");
    std::env::set_var("TERM", "zz");
    let f = db(&d, "db.txt", "dir 01;34\n*.tar=01;31 x\n");
    let r = m(&[&f]);
    assert_eq!(r.code, 0, "{}", r.err());
    // keyword 大小寫不敏感；extension key 含 '=' 時 value 只取 rest
    assert_eq!(
        entries(&first_line(&r)),
        vec!["di=01;34".to_string(), "*.tar=01;31=x".to_string()],
        "key 含 '=' 時不得重複 key"
    );
}

#[test]
fn unrecognized_keyword_and_missing_second_token() {
    let d = fresh("err");
    std::env::set_var("TERM", "zz");
    let f = db(&d, "db.txt", "DIR 01;34\nFOO 1\n");
    let r = m(&[&f]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), "dircolors: unrecognized keyword FOO");
    assert_eq!(r.out(), "", "錯誤時不得輸出 LS_COLORS");

    let g = db(&d, "bad.txt", "DIR 01;34\nDIR\n");
    let r = m(&[&g]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("invalid line;  missing second token"),
        "兩個空格：{}",
        r.err()
    );
    assert!(r.err().contains("bad.txt"), "{}", r.err());
    assert!(r.err().contains("2"), "行號須為 2：{}", r.err());

    // keyword 含 '=' 也走同一條錯誤路徑（整串出現）
    let h = db(&d, "kw.txt", "DI=v x\n");
    let r = m(&[&h]);
    assert_eq!(r.code, 1);
    assert_eq!(r.err().trim_end(), "dircolors: unrecognized keyword DI=v");
}

#[test]
fn operand_and_file_errors() {
    let d = fresh("op");
    std::env::set_var("TERM", "zz");
    let a = db(&d, "a.txt", "DIR 01;34\n");
    let b = db(&d, "b.txt", ".tar 01;31\n");

    let r = m(&[&a, &b]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().lines().next().unwrap(),
        format!("dircolors: extra operand '{}'", b)
    );
    assert!(r.err().contains("Try '"));

    let missing = "/nonexistent-dircolors-file";
    let r = m(&[missing]);
    assert_eq!(r.code, 1);
    assert_eq!(
        r.err().trim_end(),
        format!("dircolors: {}: No such file or directory (os error 2)", missing)
    );

    // 不存在的目錄當檔案
    let dir = d.to_string_lossy().into_owned();
    let r = m(&[&dir]);
    assert_eq!(r.code, 1, "{}", r.out());

    // 不支援的選項：clap 風格且帶 tip
    let r = m(&["-o", "out", &a]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().starts_with("error: unexpected argument '-o' found"),
        "{}",
        r.err()
    );
    assert!(r.err().contains("tip: to pass '-o' as a value, use '-- -o'"));

    let r = m(&["--print"]);
    assert_eq!(r.code, 1);
    assert!(
        r.err().contains("tip: a similar argument exists: '--print-database'"),
        "{}",
        r.err()
    );
}

#[test]
fn print_ls_colors_escape_format() {
    std::env::set_var("TERM", "xterm-256color");
    let r = m(&["--print-ls-colors"]);
    assert_eq!(r.code, 0, "{}", r.err());
    let out = r.out();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "\x1b[0mrs\t0\x1b[0m");
    assert_eq!(lines[1], "\x1b[01;34mdi\t01;34\x1b[0m");
    assert!(lines.len() > 40);
    assert!(lines.iter().all(|l| l.starts_with("\x1b[") && l.ends_with("\x1b[0m")));

    // TERM 未設定 → 無條目 → 無輸出
    std::env::remove_var("TERM");
    let r = m(&["--print-ls-colors"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out(), "");
    std::env::set_var("TERM", "xterm-256color");
}

#[test]
fn help_and_version() {
    let r = m(&["--help"]);
    assert_eq!(r.code, 0);
    assert_eq!(
        r.out().lines().next().unwrap(),
        "Output commands to set the LS_COLORS environment variable."
    );
    assert!(r.out().contains("Usage: dircolors [OPTION]... [FILE]"));
    for o in ["-b, --sh", "-c, --csh", "-p, --print-database", "--print-ls-colors"] {
        assert!(r.out().contains(o), "help 須列 {}", o);
    }

    let r = m(&["-V"]);
    assert_eq!(r.code, 0);
    assert_eq!(r.out().trim_end(), "dircolors (uutils coreutils) 0.10.0");
}

#[test]
fn fuzz_database_entries_and_filter() {
    let mut rng = t::Rng::new(20261111);
    // 本機實作識得的 keyword（docs/assumptions.md）
    let keywords = [
        ("RESET", "rs"),
        ("DIR", "di"),
        ("LINK", "ln"),
        ("MULTIHARDLINK", "mh"),
        ("FIFO", "pi"),
        ("SOCK", "so"),
        ("DOOR", "do"),
        ("BLK", "bd"),
        ("CHR", "cd"),
        ("ORPHAN", "or"),
        ("MISSING", "mi"),
        ("SETUID", "su"),
        ("SETGID", "sg"),
        ("CAPABILITY", "ca"),
        ("STICKY", "st"),
        ("OTHER_WRITABLE", "ow"),
        ("STICKY_OTHER_WRITABLE", "tw"),
        ("EXEC", "ex"),
        ("NORMAL", "fi"),
        ("FILE", "fi"),
        ("PIPE", "pi"),
        ("LEFT", "lc"),
        ("RIGHT", "rc"),
    ];
    let exts = [".tar", ".gz", ".zip", ".log", ".rs", ".txt"];

    for k in 0..24 {
        let d = fresh(&format!("fz{}", k));
        let term = format!("term{}", k);
        std::env::set_var("TERM", &term);
        std::env::remove_var("COLORTERM");

        let mut body = String::new();
        let mut expect: Vec<String> = Vec::new();
        let n = 1 + (rng.below(5) as usize);
        for _ in 0..n {
            let (kw, code) = keywords[rng.below(keywords.len() as u64) as usize];
            let val = format!("{};{}", rng.below(8), rng.below(8));
            let use_ext = rng.below(2) == 1;
            if use_ext {
                let e = exts[rng.below(exts.len() as u64) as usize];
                body.push_str(&format!("{} {}\n", e, val));
                expect.push(format!("*{}={}", e, val));
            } else {
                body.push_str(&format!("{} {}\n", kw, val));
                expect.push(format!("{}={}", code, val));
            }
        }
        // 結尾加一條 filter 行：不得影響前面的條目
        if rng.below(2) == 1 {
            body.push_str("TERM\tnomatch*\n");
        }
        let f = db(&d, "db.txt", &body);
        let r = m(&[&f]);
        assert_eq!(r.code, 0, "case {}: {}", k, r.err());
        assert_eq!(
            entries(&first_line(&r)),
            expect,
            "case {}: entry 順序/值（TERM={}）",
            k,
            term
        );
        assert_eq!(r.out().lines().count(), 2, "case {} 預設 sh 格式", k);
        assert_eq!(r.out().lines().nth(1), Some("export LS_COLORS"));
    }

    // 非法 keyword 必須 rc1 且訊息含整個 key
    for k in 0..8 {
        let d = fresh(&format!("bad{}", k));
        std::env::set_var("TERM", "zz");
        let bogus = format!("zz{}", k);
        let f = db(&d, "db.txt", &format!("DIR 01;34\n{} 1\n", bogus));
        let r = m(&[&f]);
        assert_eq!(r.code, 1, "case {}: {}", k, r.out());
        assert_eq!(
            r.err().trim_end(),
            format!("dircolors: unrecognized keyword {}", bogus)
        );
        assert_eq!(r.out(), "", "case {}: 錯誤時不得輸出", k);
    }
    std::env::remove_var("TERM");
}
