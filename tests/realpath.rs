//! realpath(1) e2e tests（guest 內執行）。
//! 行為與本機參考實作（uutils coreutils 0.10.0）逐 case 對 diff 過。
use coreutils::testutil as t;
use std::os::unix::fs::symlink;

const D: &str = "/tmp/rp";

fn setup_once() {
    use std::sync::Once;
    static ON: Once = Once::new();
    ON.call_once(|| {
        std::fs::remove_dir_all(D).ok();
        std::fs::create_dir_all(format!("{D}/a")).unwrap();
        std::fs::write(format!("{D}/a/b"), b"x").unwrap();
        std::fs::create_dir_all(format!("{D}/other")).unwrap();
        std::fs::create_dir_all(format!("{D}/d")).unwrap();
        symlink(format!("{D}/other"), format!("{D}/d/lsym")).unwrap();
        symlink(format!("{D}/a"), format!("{D}/lnk")).unwrap();
        symlink(format!("{D}/gone"), format!("{D}/dang")).unwrap();
        symlink(format!("{D}/cyc"), format!("{D}/cyc")).unwrap();
    });
}

fn rp(args: &[&str]) -> t::Out {
    setup_once();
    t::run("realpath", args, "")
}

#[test]
fn default_canonicalize() {
    let o = rp(&[&format!("{D}/a"), &format!("{D}/lnk")]);
    assert_eq!(o.out(), format!("{D}/a\n{D}/a\n"));
    // 尾斜線不報錯（參考實作 quirk：regular file 加尾斜線也照樣解析）。
    assert_eq!(rp(&[&format!("{D}/a/")]).out(), format!("{D}/a\n"));
    assert_eq!(rp(&[&format!("{D}/a/b/")]).out(), format!("{D}/a/b\n"));
    // 預設（-E）：最後一段可不存在。
    assert_eq!(rp(&[&format!("{D}/dang")]).out(), format!("{D}/gone\n"));
    // 中段不存在 → 錯誤。
    let o = rp(&[&format!("{D}/dang/deep")]);
    assert_eq!(o.code, 1);
    assert_eq!(o.err(), format!("realpath: {D}/dang/deep: No such file or directory\n"));
    // symlink 自環 → ELOOP。
    let o = rp(&[&format!("{D}/cyc")]);
    assert_eq!(o.code, 1);
    assert_eq!(o.err(), format!("realpath: {D}/cyc: Too many levels of symbolic links\n"));
    // 根與特殊形式。
    let o = rp(&["//", "/.", "/..", "/"]);
    assert_eq!(o.out(), "/\n/\n/\n/\n");
}

#[test]
fn mode_flags() {
    // -e：全部段都要存在。
    let o = rp(&["-e", &format!("{D}/dang")]);
    assert_eq!(o.code, 1);
    assert_eq!(o.err(), format!("realpath: {D}/dang: No such file or directory\n"));
    assert_eq!(rp(&["-e", &format!("{D}/a/")]).code, 0);
    // -m：全部可不存在。
    let o = rp(&["-m", &format!("{D}/dang/deep/deeper")]);
    assert_eq!(o.out(), format!("{D}/gone/deep/deeper\n"));
    // -E 顯式 = 預設。
    assert_eq!(rp(&["-E", &format!("{D}/dang")]).out(), format!("{D}/gone\n"));
    // -s：純詞法化，不解析 symlink。
    let o = rp(&["-s", &format!("{D}/lnk")]);
    assert_eq!(o.out(), format!("{D}/lnk\n"));
    let o = rp(&["-s", &format!("{D}/nonexistent/../x")]);
    assert_eq!((o.code, o.out()), (0, format!("{D}/x\n")));
}

#[test]
fn quiet_quirk() {
    // 參考實作（uutils 0.10.0）：-q 不只抑制訊息，exit code 也歸 0。
    let o = rp(&["-q", "-e", &format!("{D}/dang")]);
    assert_eq!((o.code, o.out(), o.err()), (0, String::new(), String::new()));
    // 沒有 -q 時同一案例是 rc1。
    assert_eq!(rp(&["-e", &format!("{D}/dang")]).code, 1);
    // --quiet 長選項。
    assert_eq!(rp(&["--quiet", "-m", "-e", &format!("{D}/dang")]).code, 0);
}

#[test]
fn logical_vs_physical() {
    // {D}/d/lsym -> {D}/other：`..` 語意取決於 -L/-P。
    let o = rp(&["-L", &format!("{D}/d/lsym/..")]);
    assert_eq!(o.out(), format!("{D}/d\n"));
    let o = rp(&["-P", &format!("{D}/d/lsym/..")]);
    assert_eq!(o.out(), format!("{D}\n"));
    // 兩者解析 symlink 本身則相同。
    assert_eq!(rp(&["-L", &format!("{D}/d/lsym")]).out(), format!("{D}/other\n"));
    assert_eq!(rp(&["-P", &format!("{D}/d/lsym")]).out(), format!("{D}/other\n"));
}

#[test]
fn zero_separator() {
    let o = rp(&["-z", &format!("{D}/a"), &format!("{D}/lnk")]);
    assert_eq!(o.out_bytes(), format!("{D}/a\0{D}/a\0").into_bytes());
    assert_eq!(rp(&["--zero", &format!("{D}/a")]).out_bytes(), format!("{D}/a\0").into_bytes());
}

#[test]
fn relative_options() {
    // --relative-to：輸出相對於 DIR。
    let o = rp(&["--relative-to", D, &format!("{D}/a/b")]);
    assert_eq!(o.out(), "a/b\n");
    let o = rp(&["--relative-to=/tmp", &format!("{D}/a/b")]);
    assert_eq!(o.out(), "rp/a/b\n");
    // --relative-base：DEST 在 BASE 之下 → 相對；不在 → 絕對。
    let o = rp(&["--relative-base", D, &format!("{D}/a/b")]);
    assert_eq!(o.out(), "a/b\n");
    let o = rp(&["--relative-base", D, "/tmp/other_zz"]);
    assert_eq!(o.out(), "/tmp/other_zz\n");
    // 兩者併用：DEST 在 BASE 之下 → 相對（以 relative-to 為基準），否則絕對。
    let o = rp(&["--relative-to", &format!("{D}/a"), "--relative-base", D, &format!("{D}/a/b")]);
    assert_eq!(o.out(), "b\n");
    let o = rp(&["--relative-to", &format!("{D}/a"), "--relative-base", "/no_such_base_zz", &format!("{D}/a/b")]);
    assert_eq!(o.out(), format!("{D}/a/b\n"));
    // 基準本身不存在也可（依詞法解析）。
    let o = rp(&["--relative-to", &format!("{D}/noexist_zz"), &format!("{D}/a")]);
    assert_eq!((o.code, o.out()), (0, "../a\n".to_string()));
    // 同路徑 → `.`。
    let o = rp(&["--relative-to", &format!("{D}/a"), &format!("{D}/a")]);
    assert_eq!(o.out(), ".\n");
    // 選項缺值 → rc2。
    let o = rp(&["--relative-to"]);
    assert_eq!(o.code, 2);
    assert!(o.err().contains("--relative-to"));
    let o = rp(&["--relative-base"]);
    assert_eq!(o.code, 2);
}

#[test]
fn option_errors_and_meta() {
    let o = rp(&[]);
    assert_eq!((o.code, o.err()), (1, "realpath: missing operand\n".to_string()));
    let o = rp(&["--bogus", "x"]);
    assert_eq!(o.code, 2);
    assert!(o.err().starts_with("realpath: unrecognized option '--bogus'"));
    let o = rp(&["-x", "x"]);
    assert_eq!(o.code, 2);
    assert!(o.err().contains("invalid option -- 'x'"));
    let o = rp(&["--help"]);
    assert_eq!(o.code, 0);
    assert!(o.out().starts_with("Usage: realpath"));
    assert_eq!(rp(&["-h"]).code, 0);
    assert_eq!(rp(&["--version"]).out(), "realpath (uutils coreutils) 0.10.0\n");
    assert_eq!(rp(&["-V"]).out(), "realpath (uutils coreutils) 0.10.0\n");
    // `--` 之後視為 operand：相對名稱末段可不存在 → rc0。
    let o = rp(&["--", "noexist_zz"]);
    assert_eq!(o.code, 0);
    assert!(o.out().ends_with("noexist_zz\n"));
}

#[test]
fn write_error_sets_status() {
    setup_once();
    let mut fw = t::FailWriter::new(1);
    let mut err = Vec::new();
    let code = t::run_with("realpath", &[&format!("{D}/a"), &format!("{D}/lnk")], &[], &mut fw, &mut err);
    assert_eq!(code, 1);
}

/// fuzz 不變量：-m 的輸出一定是正規絕對路徑（無 //、/./、/../、尾斜線），
/// 且 -s 與 -m 對不含 symlink 的隨機路徑結果相同。
#[test]
fn fuzz_output_is_canonical() {
    setup_once();
    let mut rng = t::Rng::new(0x72705f66757a7a);
    for _ in 0..200 {
        let ncomp = 1 + rng.below(5);
        let mut parts: Vec<String> = Vec::new();
        for _ in 0..ncomp {
            match rng.below(6) {
                0 => parts.push(".".to_string()),
                1 => parts.push("..".to_string()),
                2 => parts.push(String::new()), // 多餘斜線
                _ => {
                    let len = 1 + rng.below(4);
                    let mut s = String::new();
                    for _ in 0..len {
                        s.push((b'a' + rng.below(4) as u8) as char);
                    }
                    parts.push(s);
                }
            }
        }
        let name = format!("{D}/{}", parts.join("/"));
        let o = rp(&["-m", name.as_str()]);
        assert_eq!(o.code, 0, "name={name}");
        let out = o.out();
        let out = out.trim_end_matches('\n');
        assert!(out.starts_with('/'), "name={name} out={out}");
        assert!(!out.contains("//"), "name={name} out={out}");
        assert!(!out.ends_with('/') || out == "/", "name={name} out={out}");
        let comps: Vec<&str> = out.split('/').collect();
        assert!(!comps.iter().any(|c| *c == "." || *c == ".."), "name={name} out={out}");
        // 隨機路徑不含 symlink：-s（詞法）與 -m 結果應相同。
        let s = rp(&["-s", name.as_str()]);
        assert_eq!(s.out(), out.to_string() + "\n", "name={name}");
    }
}
