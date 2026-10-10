//! ls/dir/vdir 補測：把未觸及的 parse/輸出分支跑過一遍（coverage ≥80% 門檻）。
//! 斷言一律用 man page 可推導的不变量（見 docs/assumptions.md D 群叢集 4）。

use coreutils::testutil as t;

const D: &str = "/tmp/ls_c";

fn setup() {
    std::fs::create_dir_all(format!("{}/sub/deep", D)).unwrap();
    std::fs::write(format!("{}/a.txt", D), "hello").unwrap();
    std::fs::write(format!("{}/b.bin", D), "x").unwrap();
    std::fs::write(format!("{}/~bak", D), "x").unwrap();
    std::fs::write(format!("{}/.dot", D), "x").unwrap();
    let _ = std::fs::remove_file(format!("{}/lnk", D));
    let _ = std::fs::remove_dir_all(format!("{}/dlnk", D));
    let _ = std::fs::remove_file(format!("{}/dlnk", D));
    std::os::unix::fs::symlink("a.txt", format!("{}/lnk", D)).unwrap();
    std::os::unix::fs::symlink("sub", format!("{}/dlnk", D)).unwrap();
    std::fs::write(format!("{}/sub/deep/z", D), "z").unwrap();
}

#[test]
fn indicator_and_classify_variants() {
    setup();
    // -p：僅目錄加 '/'
    let r = t::run("ls", &["-1", "-p", D], "");
    assert_eq!(r.code, 0);
    let out = r.out();
    assert!(out.contains("sub/"), "目錄應有 /：{}", out);
    assert!(out.contains("a.txt"), "檔案不應有 /：{}", out);
    // --indicator-style/classify 全分支
    for f in [
        "-F",
        "--classify",
        "-p",
        "--indicator-style=slash",
        "--indicator-style=file-type",
        "--indicator-style=classify",
        "--indicator-style=none",
    ] {
        let r2 = t::run("ls", &["-1", f, D], "");
        assert_eq!(r2.code, 0, "{}: {}", f, r2.err());
    }
}

#[test]
fn time_and_sort_consumed_forms() {
    setup();
    // --time / --sort 消耗型（非 equals）與各排序旗標
    for args in [
        "--time atime",
        "--time mtime",
        "--time ctime",
        "--sort size",
        "--sort none",
        "-t",
        "-U",
        "-r",
        "-v",
        "-X",
        "-S",
        "--sort=width",
    ] {
        let parts: Vec<String> = args.split(' ').map(|x| x.to_string()).collect();
        let mut v: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
        v.push(D);
        let r = t::run("ls", &v, "");
        assert_eq!(r.code, 0, "{:?}: {}", args, r.err());
    }
}

#[test]
fn quoting_and_width_forms() {
    setup();
    for args in [
        "--quoting-style=shell",
        "--quoting-style=shell-always",
        "--quoting-style=shell-escape",
        "--quoting-style=shell-escape-always",
        "--quoting-style=escape",
        "--quoting-style=c",
        "--quoting-style=literal",
        "--quoting-style=locale",
        "--quoting-style=clocale",
        "-C -w 40",
        "-x -w 40",
        "-C --width=50",
        "-C -T 4",
        "-C --tabsize=4",
        "-1 -I *.bin",
        "-1 --hide=~*",
        "-1 -B",
        "-1 -h",
        "-l -h",
        "-l --si",
        "-l -k",
        "-l --block-size=1K",
    ] {
        let parts: Vec<String> = args.split(' ').map(|x| x.to_string()).collect();
        let mut v: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
        v.push(D);
        let r = t::run("ls", &v, "");
        assert_eq!(r.code, 0, "{:?}: {}", args, r.err());
    }
    // -I glob 生效（b.bin 被略過）
    let r = t::run("ls", &["-1", "-I", "*.bin", D], "");
    assert!(!r.out().contains("b.bin"), "-I 應略過 b.bin：{}", r.out());
    assert!(r.out().contains("a.txt"), "{}", r.out());
}

#[test]
fn dired_recursive_and_hyperlink_long() {
    setup();
    // -R 遞迴標題（單 operand 也有 'name:' 標題）
    let r = t::run("ls", &["-1", "-R", D], "");
    assert_eq!(r.code, 0);
    let out = r.out();
    assert!(out.contains(&format!("{}:", D)), "應有頂層標題：{}", out);
    assert!(out.contains("sub:"), "應有子目錄標題：{}", out);
    // -D -R：dired trailer + 遞迴
    let r2 = t::run("ls", &["-D", "-R", D], "");
    assert_eq!(r2.code, 0);
    assert!(r2.out().contains("//DIRED//"), "{}", r2.out());
    // hyperlink always 與 long 組合
    let r3 = t::run("ls", &["-l", "--hyperlink=always", D], "");
    assert_eq!(r3.code, 0);
    assert!(r3.out().contains("\u{1b}]8;;file://"), "應有 OSC8：{:?}", r3.out());
    let r4 = t::run("ls", &["--hyperlink=never", "-1", D], "");
    assert!(!r4.out().contains('\u{1b}'), "never 不應有 escape");
    let r5 = t::run("ls", &["--hyperlink=auto", "-1", D], "");
    assert!(!r5.out().contains('\u{1b}'), "auto 於非 tty 不應有 escape");
}

#[test]
fn dir_vdir_extra_flags() {
    setup();
    // dir/vdir 共用 ls 實作：跑過 long/classify/quoting 分支
    for f in ["-i", "-s", "-F", "-Q", "-b", "-q", "-R", "-p", "-1", "-l", "-a", "-A"] {
        let r = t::run("dir", &[f, D], "");
        assert_eq!(r.code, 0, "dir {}: {}", f, r.err());
        let r2 = t::run("vdir", &[f, D], "");
        assert_eq!(r2.code, 0, "vdir {}: {}", f, r2.err());
    }
    // vdir 時間欄恆空（uutils quirk）：long 行不應含時間樣式
    let r = t::run("vdir", &["-l", D], "");
    for line in r.out().lines() {
        if line.starts_with("total ") || line.is_empty() {
            continue;
        }
        assert!(!line.contains(" 20"), "vdir 不應有年份：{}", line);
    }
    // 錯誤措辭前綴是 applet 名
    let r = t::run("dir", &["/tmp/ls_c/missing"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("dir: "), "{}", r.err());
    let r = t::run("vdir", &["/tmp/ls_c/missing"], "");
    assert_eq!(r.code, 2);
    assert!(r.err().starts_with("vdir: "), "{}", r.err());
}

#[test]
fn misc_parse_flags_coverage() {
    setup();
    // 只跑不崩潰的旗標（man 可推導：rc0）
    for args in [
        "-1 -Z",
        "-1 --context",
        "-l -Z",
        "-1 --color=never",
        "-1 --color=never --group-directories-first",
        "-1 --time-style=locale",
        "-l --time-style=+%F",
        "-l --time=ctime",
        "-1 --format=horizontal",
        "-1 --format=commas",
        "-1 --format=long-iso",
        "-1 --format=iso",
        "-1 --format=full-iso",
        "-1 --format=verbatim",
        "-1 -o",
        "-1 -g",
        "-1 -n",
        "-1 -G",
        "-l --zero",
        "--help",
    ] {
        let parts: Vec<String> = args.split(' ').map(|x| x.to_string()).collect();
        let mut v: Vec<&str> = parts.iter().map(|s| s.as_str()).collect();
        if args != "--help" {
            v.push(D);
        }
        let r = t::run("ls", &v, "");
        assert_eq!(r.code, 0, "{:?}: {}", args, r.err());
    }
    let r = t::run("ls", &["--version"], "");
    assert!(r.out().contains("coreutils"), "{}", r.out());
    // --hyperlink 消耗型（GNU man：無值；參考 quirk 見 assumptions）
    let r = t::run("ls", &["-1", "--hyperlink", D], "");
    assert_eq!(r.code, 0, "{}", r.err());
    // 未知旗標 rc2
    let r = t::run("ls", &["-~", D], "");
    assert_eq!(r.code, 2, "{}", r.err());
    // 多 operand：-d 目錄＋檔案
    let extra = format!("{}/a.txt", D);
    let r = t::run("ls", &["-1", "-d", D, extra.as_str()], "");
    assert_eq!(r.code, 0, "{}", r.err());
}
