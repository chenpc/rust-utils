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
pub mod chown_common;
pub mod exec;
pub mod pathname;
pub mod passwd;

pub mod sys;
pub mod utmp;
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
        "arch" => Some(applets::arch::run),
        "b2sum" => Some(applets::b2sum::run),
        "b3sum" => Some(applets::b3sum::run),
        "base32" => Some(applets::base32::run),
        "base64" => Some(applets::base64::run),
        "basename" => Some(applets::basename::run),
        "basenc" => Some(applets::basenc::run),
        "cat" => Some(applets::cat::run),
        "chcon" => Some(applets::chcon::run),
        "chgrp" => Some(applets::chgrp::run),
        "chmod" => Some(applets::chmod::run),
        "chown" => Some(applets::chown::run),
        "chroot" => Some(applets::chroot::run),
        "cksum" => Some(applets::cksum::run),
        "comm" => Some(applets::comm::run),
        "cp" => Some(applets::cp::run),
        "csplit" => Some(applets::csplit::run),
        "cut" => Some(applets::cut::run),
        "date" => Some(applets::date::run),
        "dd" => Some(applets::dd::run),
        "df" => Some(applets::df::run),
        "dir" => Some(applets::ls::run_dir),
        "dircolors" => Some(applets::dircolors::run),
        "dirname" => Some(applets::dirname::run),
        "du" => Some(applets::du::run),
        "echo" => Some(applets::echo::run),
        "env" => Some(applets::env::run),
        "expand" => Some(applets::expand::run),
        "expr" => Some(applets::expr::run),
        "factor" => Some(applets::factor::run),
        "false" => Some(applets::r#false::run),
        "fmt" => Some(applets::fmt::run),
        "fold" => Some(applets::fold::run),
        "groups" => Some(applets::groups::run),
        "hashsum" => Some(applets::hashsum::run),
        "head" => Some(applets::head::run),
        "hostid" => Some(applets::hostid::run),
        "id" => Some(applets::id::run),
        "init" => Some(applets::init::run),
        "install" => Some(applets::install::run),
        "join" => Some(applets::join::run),
        "link" => Some(applets::link::run),
        "ln" => Some(applets::ln::run),
        "logname" => Some(applets::logname::run),
        "ls" => Some(applets::ls::run_ls),
        "md5sum" => Some(applets::md5sum::run),
        "mkdir" => Some(applets::mkdir::run),
        "mkfifo" => Some(applets::mkfifo::run),
        "mknod" => Some(applets::mknod::run),
        "mktemp" => Some(applets::mktemp::run),
        "mv" => Some(applets::mv::run),
        "nice" => Some(applets::nice::run),
        "nl" => Some(applets::nl::run),
        "nohup" => Some(applets::nohup::run),
        "nproc" => Some(applets::nproc::run),
        "numfmt" => Some(applets::numfmt::run),
        "od" => Some(applets::od::run),
        "paste" => Some(applets::paste::run),
        "pathchk" => Some(applets::pathchk::run),
        "pinky" => Some(applets::pinky::run),
        "pr" => Some(applets::pr::run),
        "printenv" => Some(applets::printenv::run),
        "printf" => Some(applets::printf::run),
        "ptx" => Some(applets::ptx::run),
        "pwd" => Some(applets::pwd::run),
        "readlink" => Some(applets::readlink::run),
        "realpath" => Some(applets::realpath::run),
        "relpath" => Some(applets::relpath::run),
        "rm" => Some(applets::rm::run),
        "rmdir" => Some(applets::rmdir::run),
        "runcon" => Some(applets::runcon::run),
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
        "shred" => Some(applets::shred::run),
        "shuf" => Some(applets::shuf::run),
        "sleep" => Some(applets::sleep::run),
        "sort" => Some(applets::sort::run),
        "split" => Some(applets::split::run),
        "stat" => Some(applets::stat::run),
        "stdbuf" => Some(applets::stdbuf::run),
        "stty" => Some(applets::stty::run),
        "sum" => Some(applets::sum::run),
        "sync" => Some(applets::sync::run),
        "tac" => Some(applets::tac::run),
        "tail" => Some(applets::tail::run),
        "tee" => Some(applets::tee::run),
        "test" => Some(applets::test::run),
        "timeout" => Some(applets::timeout::run),
        "touch" => Some(applets::touch::run),
        "tr" => Some(applets::tr::run),
        "true" => Some(applets::r#true::run),
        "truncate" => Some(applets::truncate::run),
        "tsort" => Some(applets::tsort::run),
        "tty" => Some(applets::tty::run),
        "uname" => Some(applets::uname::run),
        "unexpand" => Some(applets::unexpand::run),
        "uniq" => Some(applets::uniq::run),
        "unlink" => Some(applets::unlink::run),
        "users" => Some(applets::users::run),
        "vdir" => Some(applets::ls::run_vdir),
        "wc" => Some(applets::wc::run),
        "who" => Some(applets::who::run),
        "whoami" => Some(applets::whoami::run),
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
