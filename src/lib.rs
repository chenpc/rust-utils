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

pub mod sys;
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
        "b2sum" => Some(applets::b2sum::run),
        "b3sum" => Some(applets::b3sum::run),
        "base32" => Some(applets::base32::run),
        "base64" => Some(applets::base64::run),
        "basename" => Some(applets::basename::run),
        "basenc" => Some(applets::basenc::run),
        "cat" => Some(applets::cat::run),
        "cksum" => Some(applets::cksum::run),
        "comm" => Some(applets::comm::run),
        "csplit" => Some(applets::csplit::run),
        "cut" => Some(applets::cut::run),
        "dd" => Some(applets::dd::run),
        "df" => Some(applets::df::run),
        "dirname" => Some(applets::dirname::run),
        "du" => Some(applets::du::run),
        "echo" => Some(applets::echo::run),
        "expand" => Some(applets::expand::run),
        "expr" => Some(applets::expr::run),
        "false" => Some(applets::r#false::run),
        "fmt" => Some(applets::fmt::run),
        "fold" => Some(applets::fold::run),
        "hashsum" => Some(applets::hashsum::run),
        "head" => Some(applets::head::run),
        "init" => Some(applets::init::run),
        "join" => Some(applets::join::run),
        "link" => Some(applets::link::run),
        "ln" => Some(applets::ln::run),
        "md5sum" => Some(applets::md5sum::run),
        "mkdir" => Some(applets::mkdir::run),
        "nl" => Some(applets::nl::run),
        "numfmt" => Some(applets::numfmt::run),
        "od" => Some(applets::od::run),
        "paste" => Some(applets::paste::run),
        "pathchk" => Some(applets::pathchk::run),
        "printf" => Some(applets::printf::run),
        "ptx" => Some(applets::ptx::run),
        "readlink" => Some(applets::readlink::run),
        "realpath" => Some(applets::realpath::run),
        "relpath" => Some(applets::relpath::run),
        "rmdir" => Some(applets::rmdir::run),
        "seq" => Some(applets::seq::run),
        "sh" => Some(applets::sh::run),
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
        "shuf" => Some(applets::shuf::run),
        "sort" => Some(applets::sort::run),
        "split" => Some(applets::split::run),
        "sum" => Some(applets::sum::run),
        "sync" => Some(applets::sync::run),
        "tac" => Some(applets::tac::run),
        "tail" => Some(applets::tail::run),
        "tee" => Some(applets::tee::run),
        "test" => Some(applets::test::run),
        "tr" => Some(applets::tr::run),
        "true" => Some(applets::r#true::run),
        "truncate" => Some(applets::truncate::run),
        "tsort" => Some(applets::tsort::run),
        "unexpand" => Some(applets::unexpand::run),
        "uniq" => Some(applets::uniq::run),
        "unlink" => Some(applets::unlink::run),
        "wc" => Some(applets::wc::run),
        "yes" => Some(applets::yes::run),
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
