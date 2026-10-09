//! coreutils — 單一 multi-call binary
//!
//! 行為模型只從 man page 推導（docs/man/ 的本機 mirror 為基，缺的 10 個實作時 web search）。
//! 禁止參照 GNU / uutils 的 source code。
//!
//! `dispatch` 是 applet 的統一入口：main.rs 用它（argv[0] 決定 applet），
//! 測試也用它（in-process），這樣 coverage 能涵蓋真正的執行路徑。

pub mod applets;
pub mod basen;
pub mod hashsum;
pub mod pathname;
pub mod regex;
pub mod testutil;

use std::io::Read;
use std::io::Write;

pub type In<'a> = &'a mut dyn Read;
pub type Out<'a> = &'a mut dyn Write;

/// applet 統一簽名：args 是 applet 名稱之後的參數（不含 argv[0]）。
pub type AppletFn = fn(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32;

/// applet 註冊表。argv[0] 的 basename 或 multi-call 模式的第二個參數會走到這裡。
pub fn lookup(name: &str) -> Option<AppletFn> {
    match name {
        "[" => Some(applets::test::run_bracket),
        "base32" => Some(applets::base32::run),
        "base64" => Some(applets::base64::run),
        "basenc" => Some(applets::basenc::run),
        "basename" => Some(applets::basename::run),
        "cat" => Some(applets::cat::run),
        "comm" => Some(applets::comm::run),
        "csplit" => Some(applets::csplit::run),
        "cut" => Some(applets::cut::run),
        "dirname" => Some(applets::dirname::run),
        "echo" => Some(applets::echo::run),
        "expand" => Some(applets::expand::run),
        "expr" => Some(applets::expr::run),
        "false" => Some(applets::r#false::run),
        "fmt" => Some(applets::fmt::run),
        "fold" => Some(applets::fold::run),
        "head" => Some(applets::head::run),
        "join" => Some(applets::join::run),
        "nl" => Some(applets::nl::run),
        "numfmt" => Some(applets::numfmt::run),
        "od" => Some(applets::od::run),
        "pathchk" => Some(applets::pathchk::run),
        "paste" => Some(applets::paste::run),
        "printf" => Some(applets::printf::run),
        "ptx" => Some(applets::ptx::run),
        "realpath" => Some(applets::realpath::run),
        "readlink" => Some(applets::readlink::run),
        "seq" => Some(applets::seq::run),
        "shuf" => Some(applets::shuf::run),
        "sort" => Some(applets::sort::run),
        "split" => Some(applets::split::run),
        "tac" => Some(applets::tac::run),
        "tail" => Some(applets::tail::run),
        "tee" => Some(applets::tee::run),
        "test" => Some(applets::test::run),
        "tr" => Some(applets::tr::run),
        "true" => Some(applets::r#true::run),
        "tsort" => Some(applets::tsort::run),
        "unexpand" => Some(applets::unexpand::run),
        "uniq" => Some(applets::uniq::run),
        "wc" => Some(applets::wc::run),
        "yes" => Some(applets::yes::run),
        "sum" => Some(applets::sum::run),
        "b2sum" => Some(applets::b2sum::run),
        "b3sum" => Some(applets::b3sum::run),
        "cksum" => Some(applets::cksum::run),
        "hashsum" => Some(applets::hashsum::run),
        "md5sum" => Some(applets::md5sum::run),
        "sha1sum" => Some(applets::sha1sum::run),
        "sha224sum" => Some(applets::sha224sum::run),
        "sha256sum" => Some(applets::sha256sum::run),
        "sha3-224sum" => Some(applets::sha3_224sum::run),
        "sha3-256sum" => Some(applets::sha3_256sum::run),
        "sha3-384sum" => Some(applets::sha3_384sum::run),
        "sha3-512sum" => Some(applets::sha3_512sum::run),
        "sha384sum" => Some(applets::sha384sum::run),
        "sha3sum" => Some(applets::sha3sum::run),
        "sha512sum" => Some(applets::sha512sum::run),
        "shake128sum" => Some(applets::shake128sum::run),
        "shake256sum" => Some(applets::shake256sum::run),
        _ => None,
    }
}

pub fn dispatch(name: &str, args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    match lookup(name) {
        Some(f) => f(args, stdin, stdout, stderr),
        None => {
            writeln!(stderr, "coreutils: unknown applet '{name}'").ok();
            2
        }
    }
}
