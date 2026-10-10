//! sync(1) — Synchronize cached writes to persistent storage.
//!
//! 依據 man page（docs/man/sync.1.gz，uutils 0.10.0）：
//! - 無 FILE：sync(2) 全域 flush
//! - 有 FILE：逐檔 fsync（sync_all）
//! - `-f|--file-system`：syncfs(2)（該檔所在檔案系統）
//! - `-d|--data`：只同步資料（fdatasync/sync_data）；與 -f 互斥（參考實作拒絕）
//! - 開档失敗：`sync: error opening '<f>': <reason>`，exit 1（reason 不含 "(os error N)"）
//! - `-d` 但無 FILE：`sync: --data needs at least one argument`，exit 1
//! - 特殊檔（如 /dev/null）：fsync 回 EINVAL，但參考實測 rc=0 → 非普通檔/目錄時不 fsync，直接成功

use std::os::unix::io::AsRawFd;

use crate::sys;
use crate::{In, Out};

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut fs_mode = false;
    let mut data_only = false;
    let mut operands: Vec<String> = Vec::new();
    let mut no_more_opts = false;
    for a in args {
        if no_more_opts || (!a.starts_with('-') || a == "-") {
            operands.push(a.clone());
            continue;
        }
        if a == "--" {
            no_more_opts = true;
            continue;
        }
        if a.starts_with("--") {
            match &a[2..] {
                "file-system" => fs_mode = true,
                "data" => data_only = true,
                "help" => {
                    writeln!(stdout, "Usage: sync [-f|--file-system] [-d|--data] [-h|--help] [-V|--version] [FILE]...").ok();
                    return 0;
                }
                "version" => {
                    writeln!(stdout, "sync (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                name => {
                    writeln!(stderr, "sync: unrecognized option '--{name}'").ok();
                    return 2;
                }
            }
            continue;
        }
        for c in a[1..].chars() {
            match c {
                'f' => fs_mode = true,
                'd' => data_only = true,
                'h' => {
                    writeln!(stdout, "Usage: sync [-f|--file-system] [-d|--data] [-h|--help] [-V|--version] [FILE]...").ok();
                    return 0;
                }
                'V' => {
                    writeln!(stdout, "sync (uutils coreutils) 0.10.0").ok();
                    return 0;
                }
                c => {
                    writeln!(stderr, "sync: invalid option -- '{c}'").ok();
                    return 2;
                }
            }
        }
    }
    if fs_mode && data_only {
        writeln!(stderr, "sync: the argument '--file-system' cannot be used with '--data'").ok();
        return 1;
    }
    if operands.is_empty() {
        if data_only {
            writeln!(stderr, "sync: --data needs at least one argument").ok();
            return 1;
        }
        sys::sync_all();
        return 0;
    }
    let mut status = 0;
    for f in &operands {
        match std::fs::File::open(f) {
            Ok(file) => {
                // 參考 quirk：特殊檔（字元裝置等）fsync 會 EINVAL，但參考 rc=0 → 略過
                let md = match file.metadata() {
                    Ok(m) => Ok(m),
                    Err(_) => std::fs::metadata(f),
                };
                let syncable = md.map(|m| m.is_file() || m.is_dir()).unwrap_or(false);
                let r = if !syncable {
                    Ok(())
                } else if fs_mode {
                    sys::sync_fs(file.as_raw_fd())
                } else if data_only {
                    file.sync_data()
                } else {
                    file.sync_all()
                };
                if let Err(e) = r {
                    writeln!(stderr, "sync: error syncing '{f}': {}", crate::pathname::io_reason_pub(&e)).ok();
                    status = 1;
                }
            }
            Err(e) => {
                writeln!(stderr, "sync: error opening '{f}': {}", crate::pathname::io_reason_pub(&e)).ok();
                status = 1;
            }
        }
    }
    status
}
