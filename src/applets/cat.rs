//! cat(1) — 讀檔案或 stdin，逐 byte 複製到 stdout。
//!
//! 依據 man page（cat(1)）：
//! - 沒有檔案參數 → 讀 stdin
//! - 檔案是 `-` → 讀 stdin
//! - 檔案打不開 → 寫錯誤到 stderr，繼續處理下一個；最終退出碼 1
//!
//! 慣例：每個 applet 一個檔案，暴露 `pub fn run(args, stdin, stdout, stderr) -> i32`。

use crate::{In, Out};
use std::io::{Read, Write};

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut status = 0;

    if args.is_empty() {
        if let Err(e) = copy(stdin, stdout) {
            writeln!(stderr, "cat: standard input: {e}").ok();
            return 1;
        }
        return 0;
    }

    for a in args {
        if a == "-" {
            if let Err(e) = copy(stdin, stdout) {
                writeln!(stderr, "cat: standard input: {e}").ok();
                status = 1;
            }
            continue;
        }
        match std::fs::File::open(a) {
            Ok(mut f) => {
                if let Err(e) = copy(&mut f, stdout) {
                    writeln!(stderr, "cat: {a}: {e}").ok();
                    status = 1;
                }
            }
            Err(e) => {
                writeln!(stderr, "cat: {a}: {e}").ok();
                status = 1;
            }
        }
    }

    status
}

fn copy(r: &mut dyn Read, w: &mut dyn Write) -> std::io::Result<()> {
    let mut buf = [0u8; 8192];
    loop {
        let n = r.read(&mut buf)?;
        if n == 0 {
            break;
        }
        w.write_all(&buf[..n])?;
    }
    Ok(())
}
