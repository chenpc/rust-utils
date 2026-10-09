//! pathchk(1) e2e tests（guest 內執行）。
//! 行為與本機參考實作（uutils coreutils 0.10.0）逐 case 對 diff 過。
use coreutils::testutil as t;

fn pc(args: &[&str]) -> t::Out {
    t::run("pathchk", args, "")
}

#[test]
fn portable_names_pass() {
    let o = pc(&["foo", "a//b", "a/./b", "/", ".", "./a"]);
    assert_eq!((o.code, o.out(), o.err()), (0, String::new(), String::new()));
    // 不存在的 NAME 不是錯誤（預設模式只 stat 既有 component）。
    let o = pc(&["no_such_xyz/notthere", "/no_such_xyz_dir/foo"]);
    assert_eq!((o.code, o.out()), (0, String::new()));
    // `-` 與 `--` 之後的 operand 照單全收。
    assert_eq!(pc(&["-"]).code, 0);
    assert_eq!(pc(&["--", "--not-an-option"]).code, 0);
}

#[test]
fn default_mode_messages() {
    // 空名稱：對 '' 做 stat → ENOENT（訊息帶 applet 前綴與引號）。
    let o = pc(&[""]);
    assert_eq!(o.code, 1);
    assert_eq!(o.err(), "pathchk: '': No such file or directory\n");
    // 總長度 > 4096：只報 limit，不再碰檔案系統。
    let name = "y".to_string() + &"y".repeat(4096); // 4097 chars
    let o = pc(&[name.as_str()]);
    assert_eq!(o.code, 1);
    assert_eq!(
        o.err(),
        format!("limit 4096 exceeded by length 4097 of file name '{name}'\n")
    );
    // 256 chars：低於 4096 上限，但 component 前綴 stat 失敗 ENAMETOOLONG。
    let name2 = "z".repeat(256);
    let o = pc(&[name2.as_str()]);
    assert_eq!(o.code, 1);
    assert_eq!(o.err(), "File name too long (os error 36)\n");
    // 多個 operand：各自回報，exit 1。
    let o = pc(&["ok_name", ""]);
    assert_eq!(o.code, 1);
    assert_eq!(o.err(), "pathchk: '': No such file or directory\n");
}

#[test]
fn posix_mode_messages() {
    let o = pc(&["-p", ""]);
    assert_eq!((o.code, o.err()), (1, "empty file name\n".to_string()));
    // 非可移植字元：每個 component 只報第一個 offending 字元。
    let o = pc(&["-p", "~x~"]);
    assert_eq!(
        o.err(),
        "nonportable character '~' in file name component '~x~'\n"
    );
    // component > 14：只報第一個超限 component。
    let o = pc(&["-p", &format!("{}/{}", "a".repeat(15), "b".repeat(15))]);
    assert_eq!(
        o.err(),
        "limit 14 exceeded by length 15 of file name component 'aaaaaaaaaaaaaaa'\n"
    );
    // 總長 > 256：limit 256，此訊息不加引號。
    let name = "x".repeat(300);
    let o = pc(&["-p", name.as_str()]);
    assert_eq!(
        o.err(),
        format!("limit 256 exceeded by length 300 of file name {name}\n")
    );
    // 恰好 14 不算超限；總長 256（component 都 ≤14）也不算超限
    // （單 component 256 會先撞 component 上限，故用斜線切分測總長邊界）。
    assert_eq!(pc(&["-p", &"x".repeat(14)]).code, 0);
    let joined = vec!["x".repeat(14); 16].join("/") + "/xxxxxxxx"; // 16*15+8 = 248 chars
    assert_eq!(joined.len(), 248);
    assert_eq!(pc(&["-p", joined.as_str()]).code, 0);
    // -p 不做檔案系統檢查。
    assert_eq!(pc(&["-p", "/no_such_xyz/nothere"]).code, 0);
    // 多個 operand 各自回報。
    let o = pc(&["-p", "x", "y~z", "w a"]);
    assert_eq!(
        o.err(),
        "nonportable character '~' in file name component 'y~z'\n\
         nonportable character ' ' in file name component 'w a'\n"
    );
}

#[test]
fn hyphen_and_portability() {
    let o = pc(&["-P", "a/-x"]);
    assert_eq!(o.err(), "leading hyphen in file name component '-x'\n");
    // 無 -P 時 leading hyphen 不是錯誤。
    assert_eq!(pc(&["a/-x"]).code, 0);
    // -P 空名稱走預設分支（stat ENOENT），不是 "empty file name"。
    let o = pc(&["-P", ""]);
    assert_eq!(o.err(), "pathchk: '': No such file or directory\n");
    // --portability = -p -P：兩個 operand 各報一行。
    let o = pc(&["--portability", "a b", "q/-x"]);
    assert_eq!(
        o.err(),
        "nonportable character ' ' in file name component 'a b'\n\
         leading hyphen in file name component '-x'\n"
    );
}

#[test]
fn option_errors_and_meta() {
    let o = pc(&[]);
    assert_eq!((o.code, o.err()), (1, "pathchk: missing operand\n".to_string()));
    // clap 對 unknown option 的措辭不作為斷言（本專案 parser 採 GNU 措辭）。
    let o = pc(&["--bogus", "x"]);
    assert_eq!(o.code, 2);
    assert!(o.err().starts_with("pathchk: unrecognized option '--bogus'"));
    let o = pc(&["-x", "x"]);
    assert_eq!(o.code, 2);
    assert!(o.err().contains("invalid option -- 'x'"));
    let o = pc(&["--help"]);
    assert_eq!(o.code, 0);
    assert!(o.out().starts_with("Usage: pathchk"));
    assert_eq!(pc(&["-h"]).code, 0);
    let o = pc(&["--version"]);
    assert_eq!(o.out(), "pathchk (uutils coreutils) 0.10.0\n");
    assert_eq!(pc(&["-V"]).out(), "pathchk (uutils coreutils) 0.10.0\n");
}
