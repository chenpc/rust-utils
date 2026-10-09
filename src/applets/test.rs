//! `test` 與 `[`：依 man page（docs/man/test.1.gz）實作，同一份核心只差 argv[0]。
//!
//! 假設（記錄於 docs/assumptions.md）：
//! - 語法錯誤 → exit 2；true → 0；false → 1。
//! - `-a` / `-o` 只有在左邊已有運算式時才是運算子，否則視為字串。
//! - `-r` / `-w` 依 mode 位元判斷（owner/group/other），不是真正的 access()。
//! - `-O` / `-G` 從 /proc/self/status 讀有效 uid/gid（guest 內有 mount 的 proc）。
//! - `-t FD` 只支援 FD 0/1/2（用 std 的 IsTerminal）。

use crate::In;
use crate::Out;
use std::os::unix::fs::{FileTypeExt, MetadataExt};

struct P<'a> {
    toks: &'a [String],
    pos: usize,
}

fn truthy(s: &str) -> bool {
    !s.is_empty()
}

fn effective_ids() -> (u32, u32) {
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        let mut uid = 0;
        let mut gid = 0;
        for line in status.lines() {
            if let Some(v) = line.strip_prefix("Uid:") {
                uid = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0);
            } else if let Some(v) = line.strip_prefix("Gid:") {
                gid = v.split_whitespace().next().and_then(|x| x.parse().ok()).unwrap_or(0);
            }
        }
        return (uid, gid);
    }
    (0, 0)
}

fn can_read_or_write(md: &std::fs::Metadata, kind: i32) -> bool {
    let mode = md.mode() & 0o777;
    let bit = match kind {
        3 => 0o100,
        2 => 0o200,
        _ => 0o400,
    };
    let (uid, gid) = effective_ids();
    if md.uid() == uid {
        mode & bit != 0
    } else if md.gid() == gid {
        mode & (bit >> 3) != 0
    } else {
        mode & (bit >> 6) != 0
    }
}

fn file_test(op: &str, path: &str) -> bool {
    let lm = match std::fs::symlink_metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    match op {
        "-e" => return true,
        "-L" | "-h" => return lm.file_type().is_symlink(),
        _ => {}
    }
    let md = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    let ft = md.file_type();
    match op {
        "-b" => ft.is_block_device(),
        "-c" => ft.is_char_device(),
        "-d" => ft.is_dir(),
        "-f" => ft.is_file(),
        "-p" => ft.is_fifo(),
        "-S" => ft.is_socket(),
        "-s" => md.len() > 0,
        "-g" => md.mode() & 0o2000 != 0,
        "-k" => md.mode() & 0o1000 != 0,
        "-r" => can_read_or_write(&md, 1),
        "-w" => can_read_or_write(&md, 2),
        "-x" => can_read_or_write(&md, 3),
        "-G" => md.gid() == effective_ids().1,
        "-O" => md.uid() == effective_ids().0,
        "-N" => md.mtime() > md.atime(),
        _ => false,
    }
}

impl P<'_> {
    fn peek(&self) -> Option<&String> {
        self.toks.get(self.pos)
    }

    fn eval_or(&mut self) -> Result<bool, ()> {
        let mut left = self.eval_and()?;
        while self.peek().map(|s| s.as_str()) == Some("-o") {
            self.pos += 1;
            let right = self.eval_and()?;
            left = left || right;
        }
        Ok(left)
    }

    fn eval_and(&mut self) -> Result<bool, ()> {
        let mut left = self.eval_unary()?;
        while self.peek().map(|s| s.as_str()) == Some("-a") {
            self.pos += 1;
            let right = self.eval_unary()?;
            left = left && right;
        }
        Ok(left)
    }

    fn eval_unary(&mut self) -> Result<bool, ()> {
        let tok = match self.peek() {
            Some(t) => t.clone(),
            None => return Err(()),
        };
        if tok == "!" {
            self.pos += 1;
            return self.eval_or().map(|v| !v);
        }
        if tok == "(" {
            self.pos += 1;
            let v = self.eval_or()?;
            if self.peek().map(|s| s.as_str()) != Some(")") {
                return Err(());
            }
            self.pos += 1;
            return Ok(v);
        }
        // 一元運算子
        if is_unary_op(&tok) {
            self.pos += 1;
            let arg = match self.peek() {
                Some(t) => t.clone(),
                None => return Err(()),
            };
            self.pos += 1;
            if tok == "-n" {
                return Ok(!arg.is_empty());
            }
            if tok == "-z" {
                return Ok(arg.is_empty());
            }
            if tok == "-t" {
                let fd: i32 = arg.parse().unwrap_or(-1);
                return Ok(match fd {
                    0 => std::io::IsTerminal::is_terminal(&std::io::stdin()),
                    1 => std::io::IsTerminal::is_terminal(&std::io::stdout()),
                    2 => std::io::IsTerminal::is_terminal(&std::io::stderr()),
                    _ => false,
                });
            }
            return Ok(file_test(&tok, &arg));
        }
        // 二元：STRING op STRING / INTEGER op INTEGER
        self.pos += 1;
        let op = match self.peek() {
            Some(t) => t.clone(),
            None => return Ok(truthy(&tok)),
        };
        if !is_binary_op(&op) {
            return Ok(truthy(&tok));
        }
        self.pos += 1;
        let right = match self.peek() {
            Some(t) => t.clone(),
            None => return Err(()),
        };
        self.pos += 1;
        match op.as_str() {
            "=" => Ok(tok == right),
            "!=" => Ok(tok != right),
            "<" => Ok(tok < right),
            ">" => Ok(tok > right),
            "-ef" => {
                let a = std::fs::metadata(&tok);
                let b = std::fs::metadata(&right);
                match (a, b) {
                    (Ok(x), Ok(y)) => Ok(x.dev() == y.dev() && x.ino() == y.ino()),
                    _ => Err(()),
                }
            }
            "-nt" => {
                let a = std::fs::metadata(&tok).and_then(|m| m.modified());
                let b = std::fs::metadata(&right).and_then(|m| m.modified());
                match (a, b) {
                    (Ok(x), Ok(y)) => Ok(x > y),
                    _ => Err(()),
                }
            }
            "-ot" => {
                let a = std::fs::metadata(&tok).and_then(|m| m.modified());
                let b = std::fs::metadata(&right).and_then(|m| m.modified());
                match (a, b) {
                    (Ok(x), Ok(y)) => Ok(x < y),
                    _ => Err(()),
                }
            }
            _ => {
                let a: i64 = match tok.parse() {
                    Ok(v) => v,
                    Err(_) => return Err(()),
                };
                let b: i64 = match right.parse() {
                    Ok(v) => v,
                    Err(_) => return Err(()),
                };
                Ok(match op.as_str() {
                    "-eq" => a == b,
                    "-ne" => a != b,
                    "-lt" => a < b,
                    "-le" => a <= b,
                    "-gt" => a > b,
                    "-ge" => a >= b,
                    _ => false,
                })
            }
        }
    }
}

fn is_unary_op(s: &str) -> bool {
    matches!(
        s,
        "-n" | "-z" | "-b" | "-c" | "-d" | "-e" | "-f" | "-g" | "-G" | "-h" | "-k" | "-L" | "-N" | "-O" | "-p"
            | "-r" | "-s" | "-S" | "-t" | "-w" | "-x"
    )
}

fn is_binary_op(s: &str) -> bool {
    match s {
        "=" | "!=" | "<" | ">" | "-eq" | "-ne" | "-lt" | "-le" | "-gt" | "-ge" | "-ef" | "-nt" | "-ot" => true,
        _ => false,
    }
}

fn eval(args: &[String], name: &str, stderr: Out) -> i32 {
    if args.is_empty() {
        return 1;
    }
    let mut toks: Vec<String> = args.to_vec();
    if name == "[" {
        if toks.last().map(|s| s.as_str()) != Some("]") {
            let _ = stderr.write_all(b"[: missing ']'\n");
            return 2;
        }
        toks.pop();
    }
    if toks.is_empty() {
        return 1;
    }
    let mut p = P { toks: &toks, pos: 0 };
    match p.eval_or() {
        Ok(v) => {
            if p.pos != toks.len() {
                let _ = stderr.write_all(b"[: syntax error\n");
                return 2;
            }
            if v {
                0
            } else {
                1
            }
        }
        Err(_) => {
            let _ = stderr.write_all(b"[: syntax error\n");
            2
        }
    }
}

fn help_version(name: &str, args: &[String], stdout: Out) -> Option<i32> {
    if args.len() == 1 && (args[0] == "-h" || args[0] == "--help") {
        let text = format!("Usage: {name} EXPRESSION\n");
        if stdout.write_all(text.as_bytes()).is_err() {
            return Some(1);
        }
        return Some(0);
    }
    if args.len() == 1 && (args[0] == "-V" || args[0] == "--version") {
        let text = format!("{name} (uutils coreutils) 0.10.0\n");
        if stdout.write_all(text.as_bytes()).is_err() {
            return Some(1);
        }
        return Some(0);
    }
    None
}

pub fn run(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    if let Some(code) = help_version("test", args, stdout) {
        return code;
    }
    eval(args, "test", stderr)
}

pub fn run_bracket(args: &[String], _stdin: In, stdout: Out, stderr: Out) -> i32 {
    if let Some(code) = help_version("[", args, stdout) {
        return code;
    }
    eval(args, "[", stderr)
}
