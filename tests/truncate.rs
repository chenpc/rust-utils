//! truncate guest 測試：-s 前綴/單位、-r 基準、建立行為、錯誤措辭。

use coreutils::testutil as t;

fn fresh(tag: &str) -> std::path::PathBuf {
    let d = std::path::PathBuf::from(format!("/tmp/trt_{}_{}", tag, std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    std::env::set_current_dir(&d).unwrap();
    d
}

fn size_of(p: &str) -> u64 {
    std::fs::metadata(p).map(|m| m.len()).unwrap_or(u64::MAX)
}

#[test]
fn size_absolute_and_creation() {
    fresh("abs");
    let r = t::run("truncate", &["-s", "5", "f1"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    assert_eq!(size_of("f1"), 5);
    let r = t::run("truncate", &["-s", "0", "f1"], "");
    assert_eq!(r.code, 0);
    assert_eq!(size_of("f1"), 0);
    // 不存在的檔案會被建立
    let r = t::run("truncate", &["--size=1K", "f2"], "");
    assert_eq!(r.code, 0);
    assert_eq!(size_of("f2"), 1024);
    // --size=1KB → 1000
    let r = t::run("truncate", &["--size=1KB", "f3"], "");
    assert_eq!(r.code, 0);
    assert_eq!(size_of("f3"), 1000);
}

#[test]
fn relative_prefixes() {
    fresh("rel");
    std::fs::write("f", vec![b'x'; 10]).unwrap();
    for (spec, want) in [("+5", 15u64), ("-3", 12), ("<20", 12), (">20", 20)] {
        let r = t::run("truncate", &["-s", spec, "f"], "");
        assert_eq!((r.code, size_of("f")), (0, want), "spec {spec}");
    }
    // "=" 前綴與空字串皆 Invalid number（參考同措辭 rc1）
    for spec in ["=7", ""] {
        let r = t::run("truncate", &["-s", spec, "f"], "");
        assert_eq!((r.code, r.err().trim_end()), (1, format!("truncate: Invalid number: '{spec}'").as_str()));
    }
    // 8 位十六進位
    let r = t::run("truncate", &["-s", "0x10", "hx"], "");
    assert_eq!((r.code, size_of("hx")), (0, 16));
    // 參考不支援 / 與 %
    for spec in ["20%", "5 /3"] {
        let r = t::run("truncate", &["-s", spec, "pf"], "");
        assert_eq!((r.code, r.err().trim_end()), (1, format!("truncate: Invalid number: '{spec}'").as_str()));
    }
    // 過大值 → ERANGE 措辭
    let r = t::run("truncate", &["-s", "R", "big"], "");
    assert_eq!((r.code, r.err().starts_with("truncate: Invalid number: 'R'")), (1, true));
}

#[test]
fn reference_file() {
    fresh("ref");
    std::fs::write("t6", vec![b'x'; 7]).unwrap();
    let r = t::run("truncate", &["-r", "t6", "t7"], "");
    assert_eq!((r.code, r.stderr.is_empty()), (0, true));
    assert_eq!(size_of("t7"), 7);
    // -r 與 -s 併用：以 ref 為基準再加減
    let r = t::run("truncate", &["-r", "t6", "-s", "+2", "t8"], "");
    assert_eq!((r.code, size_of("t8")), (0, 9));
    // ref 不存在
    let r = t::run("truncate", &["-r", "noexist_zz", "x"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "truncate: cannot stat 'noexist_zz': No such file or directory"));
    // -c 僅改既有檔：接受（本實作不特別區分）
    let r = t::run("truncate", &["-c", "-s", "3", "t9"], "");
    assert_eq!((r.code, size_of("t9")), (0, 3));
    // 無 -s 無 -r
    let r = t::run("truncate", &["noflag"], "");
    assert_eq!((r.code, r.err().contains("one of -s and --reference")), (1, true));
}

#[test]
fn errors_and_opts() {
    fresh("err");
    let r = t::run("truncate", &["-s", "5", "/"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "truncate: cannot open '/' for writing: Is a directory"));
    let r = t::run("truncate", &[], "");
    assert_eq!((r.code, r.err().contains("missing operand")), (2, true));
    let r = t::run("truncate", &["-s"], "");
    assert_eq!((r.code, r.err().trim_end()), (2, "truncate: option requires an argument -- 's'"));
    let r = t::run("truncate", &["--bogus", "x"], "");
    assert_eq!((r.code, r.err().contains("unrecognized option")), (2, true));
    let r = t::run("truncate", &["-q", "x"], "");
    assert_eq!((r.code, r.err().trim_end()), (2, "truncate: invalid option -- 'q'"));
    let r = t::run("truncate", &["--help"], "");
    assert_eq!((r.code, r.out().contains("Usage: truncate")), (0, true));
    let r = t::run("truncate", &["--version"], "");
    assert_eq!((r.code, r.out().trim_end()), (0, "truncate (uutils coreutils) 0.10.0"));
    let r = t::run("truncate", &["-h"], "");
    assert_eq!((r.code, r.out().contains("Usage")), (0, true));
    let r = t::run("truncate", &["-V"], "");
    assert_eq!((r.code, r.out().contains("uutils")), (0, true));
    let r = t::run("truncate", &["--reference=", "x"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "truncate: cannot stat '': No such file or directory"));
    let r = t::run("truncate", &["--size=", "x"], "");
    assert_eq!((r.code, r.err().trim_end()), (1, "truncate: Invalid number: ''"));
}

#[test]
fn fuzz_truncate_roundtrip() {
    fresh("fz");
    let mut rng = t::Rng::new(20260730);
    for i in 0..60 {
        let n = rng.below(4096);
        let r = t::run("truncate", &["-s", &n.to_string(), "ff"], "");
        assert_eq!(r.code, 0, "case {i}");
        assert_eq!(size_of("ff"), n);
        // 加減往返
        let r = t::run("truncate", &["-s", "+7", "ff"], "");
        assert_eq!((r.code, size_of("ff")), (0, n + 7));
        let r = t::run("truncate", &["-s", "-7", "ff"], "");
        assert_eq!((r.code, size_of("ff")), (0, n));
        // --reference 複製大小
        let r = t::run("truncate", &["--reference=ff", "gg"], "");
        assert_eq!((r.code, size_of("gg")), (0, n));
    }
}
