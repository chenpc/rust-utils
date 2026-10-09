//! readlink(1) e2e tests（guest 內執行）。
//! 行為與本機參考實作（uutils coreutils 0.10.0）逐 case 對 diff 過。
use coreutils::testutil as t;
use std::os::unix::fs::symlink;

const D: &str = "/tmp/rl";

fn setup() {
    std::fs::remove_dir_all(D).ok();
    std::fs::create_dir_all(format!("{D}/dir.d")).unwrap();
    std::fs::write(format!("{D}/f.txt"), b"x").unwrap();
    symlink("two words", format!("{D}/lnk")).unwrap(); // 值含空格
    symlink("no-such-target", format!("{D}/dang")).unwrap();
    symlink("cyc", format!("{D}/cyc")).unwrap();
}

fn rl(args: &[&str]) -> t::Out {
    setup_once();
    t::run("readlink", args, "")
}

fn setup_once() {
    use std::sync::Once;
    static ON: Once = Once::new();
    ON.call_once(setup);
}

#[test]
fn default_mode_prints_link_value() {
    let o = rl(&[&format!("{D}/lnk")]);
    assert_eq!((o.code, o.out()), (0, "two words\n".to_string()));
    // dangle symlink 也印值。
    let o = rl(&[&format!("{D}/dang")]);
    assert_eq!((o.code, o.out()), (0, "no-such-target\n".to_string()));
    // 自環：印出自己的值（一跳，不展開）。
    let o = rl(&[&format!("{D}/cyc")]);
    assert_eq!((o.code, o.out()), (0, "cyc\n".to_string()));
    // 不是 symlink：無輸出、exit 1、預設不印訊息。
    let o = rl(&[&format!("{D}/f.txt")]);
    assert_eq!((o.code, o.out(), o.err()), (1, String::new(), String::new()));
    let o = rl(&[&format!("{D}/dir.d")]);
    assert_eq!((o.code, o.err()), (1, String::new()));
    // 不存在：同樣無訊息。
    let o = rl(&[&format!("{D}/noexist_zz")]);
    assert_eq!((o.code, o.err()), (1, String::new()));
    // 多 operand：各自輸出，任一失敗 → exit 1。
    let o = rl(&[&format!("{D}/lnk"), &format!("{D}/f.txt")]);
    assert_eq!((o.code, o.out()), (1, "two words\n".to_string()));
}

#[test]
fn verbose_errors() {
    let o = rl(&["-v", &format!("{D}/f.txt")]);
    assert_eq!(
        o.err(),
        format!("readlink: {D}/f.txt: Invalid argument\n")
    );
    let o = rl(&["-v", &format!("{D}/noexist_zz")]);
    assert_eq!(
        o.err(),
        format!("readlink: {D}/noexist_zz: No such file or directory\n")
    );
    let o = rl(&["-v", ""]);
    assert_eq!(o.err(), "readlink: '': No such file or directory\n");
    // -q 在 -v 之後：訊息再次被抑制，exit code 仍 1。
    let o = rl(&["-v", "-q", &format!("{D}/f.txt")]);
    assert_eq!((o.code, o.err()), (1, String::new()));
    // --verbose 長選項同行為。
    let o = rl(&["--verbose", &format!("{D}/dir.d")]);
    assert_eq!(o.err(), format!("readlink: {D}/dir.d: Invalid argument\n"));
}

#[test]
fn newline_and_zero() {
    // -n 單一 operand：不換行。
    let o = rl(&["-n", &format!("{D}/lnk")]);
    assert_eq!(o.out_bytes(), b"two words");
    // -n 多 operand：警告後照常換行。
    let o = rl(&["-n", &format!("{D}/lnk"), &format!("{D}/dang")]);
    assert_eq!(o.err(), "readlink: ignoring --no-newline with multiple arguments\n");
    assert_eq!(o.out(), "two words\nno-such-target\n");
    // -z：NUL 分隔。
    let o = rl(&["-z", &format!("{D}/lnk"), &format!("{D}/dang")]);
    assert_eq!(o.out_bytes(), b"two words\0no-such-target\0");
    // --no-newline / --zero 長選項。
    assert_eq!(rl(&["--no-newline", &format!("{D}/lnk")]).out_bytes(), b"two words");
    assert_eq!(rl(&["--zero", &format!("{D}/lnk")]).out_bytes(), b"two words\0");
}

#[test]
fn canonicalize_modes() {
    let o = rl(&["-f", &format!("{D}/lnk")]);
    assert_eq!((o.code, o.out()), (0, format!("{D}/two words\n")));
    // -f：最後一段可不存在。
    let o = rl(&["-f", &format!("{D}/dang")]);
    assert_eq!((o.code, o.out()), (0, format!("{D}/no-such-target\n")));
    let o = rl(&["-f", &format!("{D}/dang/deep")]);
    assert_eq!(o.code, 1); // 中段的 dangling 不允許
    let o = rl(&["-e", &format!("{D}/dang")]);
    assert_eq!(o.code, 1);
    let o = rl(&["-e", "-v", &format!("{D}/dang")]);
    assert_eq!(o.err(), format!("readlink: {D}/dang: No such file or directory\n"));
    // -m：全部可不存在。
    let o = rl(&["-m", &format!("{D}/dang/deep/x")]);
    assert_eq!((o.code, o.out()), (0, format!("{D}/no-such-target/deep/x\n")));
    // 自環 -f：ELOOP。
    let o = rl(&["-f", "-v", &format!("{D}/cyc")]);
    assert_eq!(o.code, 1);
    assert_eq!(o.err(), format!("readlink: {D}/cyc: Too many levels of symbolic links\n"));
    // 長選項。
    assert_eq!(rl(&["--canonicalize", &format!("{D}/lnk")]).out(), format!("{D}/two words\n"));
    assert_eq!(rl(&["--canonicalize-existing", &format!("{D}/f.txt")]).code, 0);
    assert_eq!(rl(&["--canonicalize-missing", &format!("{D}/noexist_zz/deep")]).code, 0);
}

#[test]
fn option_errors_and_meta() {
    let o = rl(&[]);
    assert_eq!((o.code, o.err()), (1, "readlink: missing operand\n".to_string()));
    let o = rl(&["--bogus", "x"]);
    assert_eq!(o.code, 2);
    assert!(o.err().starts_with("readlink: unrecognized option '--bogus'"));
    let o = rl(&["-x", "x"]);
    assert_eq!(o.code, 2);
    assert!(o.err().contains("invalid option -- 'x'"));
    let o = rl(&["--help"]);
    assert_eq!(o.code, 0);
    assert!(o.out().starts_with("Usage: readlink"));
    assert_eq!(rl(&["-h"]).code, 0);
    assert_eq!(rl(&["--version"]).out(), "readlink (uutils coreutils) 0.10.0\n");
    assert_eq!(rl(&["-V"]).out(), "readlink (uutils coreutils) 0.10.0\n");
    // `--` 之後視為 operand。
    assert_eq!(rl(&["--", "noexist_zz"]).code, 1);
}

#[test]
fn write_error_sets_status() {
    setup_once();
    let mut fw = t::FailWriter::new(1);
    let mut err = Vec::new();
    let code = t::run_with(
        "readlink",
        &["-z", &format!("{D}/lnk"), &format!("{D}/dang")],
        &[],
        &mut fw,
        &mut err,
    );
    assert_eq!(code, 1); // 第一次寫入成功、第二次失敗
}
