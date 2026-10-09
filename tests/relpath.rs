//! relpath(1) e2e tests（guest 內執行）。
//! 本機參考實作（uutils 0.10.0）不提供 relpath；行為依 GNU coreutils 文件
//! （見 docs/assumptions.md），並與 GNU info 的範例對照。
use coreutils::testutil as t;
use std::os::unix::fs::symlink;

const D: &str = "/tmp/rpz";

fn setup_once() {
    use std::sync::Once;
    static ON: Once = Once::new();
    ON.call_once(|| {
        std::fs::remove_dir_all(D).ok();
        std::fs::create_dir_all(format!("{D}/t")).unwrap();
        std::fs::create_dir_all(format!("{D}/sub/x")).unwrap();
        symlink(format!("{D}/t"), format!("{D}/l")).unwrap();
    });
}

fn r(args: &[&str]) -> t::Out {
    setup_once();
    t::run("relpath", args, "")
}

#[test]
fn absolute_operands() {
    // GNU info 範例的變體：從 BASE 走到 DEST。
    assert_eq!(r(&[&format!("{D}/t"), D]).out(), "t\n");
    assert_eq!(r(&[D, &format!("{D}/t")]).out(), "..\n");
    assert_eq!(r(&[&format!("{D}/t"), &format!("{D}/t")]).out(), ".\n");
    assert_eq!(r(&["/etc/X11", "/usr/local/bin"]).out(), "../../../etc/X11\n");
    // 詞法化：`.`、多餘 `/`、絕對路徑上的 `..` 先折疊（不碰檔案系統）。
    assert_eq!(r(&["/a/b/../c", "/a"]).out(), "c\n");
    assert_eq!(r(&["/a/./b", "/a//"]).out(), "b\n");
    // 不存在的運算元也可以（預設不做 IO）。
    assert_eq!(r(&["/no_such_zz/deep", "/no_such_zz"]).out(), "deep\n");
}

#[test]
fn strict_mode() {
    assert_eq!(r(&["-s", &format!("{D}/t"), D]).out(), "t\n");
    let o = r(&["-s", "foo", "bar"]);
    assert_eq!(o.code, 1);
    assert_eq!(o.err(), "relpath: both paths must be absolute: 'foo', 'bar'\n");
    // BASE 省略時預設 `.`，strict 下必然違反。
    let o = r(&["-s", &format!("{D}/t")]);
    assert_eq!(o.err(), "relpath: both paths must be absolute: '/tmp/rpz/t', '.'\n");
    // --strict 長選項。
    assert_eq!(r(&["--strict", "/a/b", "/a"]).out(), "b\n");
}

#[test]
fn physical_mode() {
    // -p：兩邊先解析 symlink（皆須存在）。
    assert_eq!(r(&["-p", &format!("{D}/l"), D]).out(), "t\n");
    assert_eq!(r(&["-p", &format!("{D}/l"), &format!("{D}/t")]).out(), ".\n");
    // 不存在 → 錯誤、exit 1。
    let o = r(&["-p", "/no_such_zz_a", "/no_such_zz_b"]);
    assert_eq!(o.code, 1);
    assert!(o.err().starts_with("relpath: "));
    // 對照：預設模式不解析 symlink。
    assert_eq!(r(&[&format!("{D}/l"), D]).out(), "l\n");
    // --physical 長選項。
    assert_eq!(r(&["--physical", &format!("{D}/l"), D]).out(), "t\n");
}

#[test]
fn relative_operands_use_cwd() {
    // 注意：本測試改 cwd，須與其它測試的絕對運算元互不干擾。
    setup_once();
    std::env::set_current_dir(D).unwrap();
    let o = r(&["sub/x"]); // BASE 預設 `.` → cwd；DEST 相對 → 同 cwd 下
    assert_eq!((o.code, o.out()), (0, "sub/x\n".to_string()));
    assert_eq!(r(&["t"]).out(), "t\n");
    assert_eq!(r(&["/tmp"]).out(), "..\n"); // BASE = cwd = {D}
    // GNU info 例的變體：DEST 與 BASE 都是相對名稱。
    assert_eq!(r(&["sub", "t"]).out(), "../sub\n");
    std::env::set_current_dir("/").unwrap();
}

#[test]
fn operand_and_option_errors() {
    let o = r(&[]);
    assert_eq!((o.code, o.err()), (1, "relpath: missing operand\n".to_string()));
    let o = r(&["/a", "/b", "/c"]);
    assert_eq!((o.code, o.err()), (1, "relpath: extra operand '/c'\n".to_string()));
    let o = r(&["--bogus", "/a"]);
    assert_eq!(o.code, 2);
    assert!(o.err().starts_with("relpath: unrecognized option '--bogus'"));
    let o = r(&["-q", "/a"]);
    assert_eq!(o.code, 2);
    assert!(o.err().contains("invalid option -- 'q'"));
    let o = r(&["--help"]);
    assert_eq!(o.code, 0);
    assert!(o.out().starts_with("Usage: relpath"));
    assert_eq!(r(&["-h"]).code, 0);
    assert_eq!(r(&["--version"]).out(), "relpath (uutils coreutils) 0.10.0\n");
    assert_eq!(r(&["-V"]).out(), "relpath (uutils coreutils) 0.10.0\n");
    // `--` 之後視為 operand。
    assert_eq!(r(&["--", "/a", "/b"]).out(), "../a\n");
}

/// 詞法折疊：對絕對路徑去 `.`、折 `..`（根之上停在根）。
fn lex_abs(p: &str) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in p.split('/') {
        match c {
            "" | "." => {}
            ".." => {
                out.pop();
            }
            _ => out.push(c.to_string()),
        }
    }
    out
}

/// fuzz 不變量：relpath(B..A) 從 B 套用輸出後必達 A（詞法可達性），
/// 且輸出 component 不含空、`.`，絕對運算元下 `..` 不會衝出根。
#[test]
fn fuzz_relpath_walk_reaches_dest() {
    setup_once();
    let mut rng = t::Rng::new(0x72705f6675);
    let gen = |rng: &mut t::Rng| -> String {
        let ncomp = 1 + rng.below(4);
        let mut parts: Vec<String> = Vec::new();
        for _ in 0..ncomp {
            if rng.below(5) == 0 {
                parts.push("..".to_string());
            } else {
                let len = 1 + rng.below(3);
                let mut s = String::new();
                for _ in 0..len {
                    s.push((b'a' + rng.below(3) as u8) as char);
                }
                parts.push(s);
            }
        }
        format!("/{}", parts.join("/"))
    };
    for _ in 0..200 {
        let a = gen(&mut rng);
        let b = gen(&mut rng);
        let o = r(&[a.as_str(), b.as_str()]);
        assert_eq!(o.code, 0, "a={a} b={b}");
        let out = o.out().trim_end_matches('\n').to_string();
        let mut cur = lex_abs(&b);
        if out != "." {
            for c in out.split('/') {
                assert!(!c.is_empty() && c != ".", "a={a} b={b} out={out}");
                match c {
                    ".." => {
                        cur.pop();
                    }
                    _ => cur.push(c.to_string()),
                }
            }
        }
        assert_eq!(cur, lex_abs(&a), "a={a} b={b} out={out}");
    }
}
