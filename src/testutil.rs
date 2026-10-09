//! 測試輔助：同一份 code path 的兩種跑法。
//!
//! - `run`     in-process（coverage 計在這裡，因為 test binary 內含全部 applet code）
//! - `run_bin` 真的把 binary 當子行程跑（e2e：argv[0] 分派、退出碼、stderr）
//!
//! 兩者都只在 qemu guest 內執行（規則 3）。
//!
//! 輸出保留原始 byte（不經 UTF-8 lossy），否則 binary-safe 的 applet（cat、head、tr…）
//! 的 fuzz 不變量會被字元替換破壞。

use crate::dispatch;
use std::io::Read;

pub struct Out {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}

impl Out {
    pub fn out(&self) -> String {
        String::from_utf8_lossy(&self.stdout).into_owned()
    }
    pub fn err(&self) -> String {
        String::from_utf8_lossy(&self.stderr).into_owned()
    }
    pub fn out_bytes(&self) -> Vec<u8> {
        self.stdout.clone()
    }
}

pub fn run(name: &str, args: &[&str], input: &str) -> Out {
    run_bytes(name, args, input.as_bytes())
}

pub fn run_bytes(name: &str, args: &[&str], input: &[u8]) -> Out {
    let mut stdin = input;
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let v: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    let code = dispatch(name, &v, &mut stdin, &mut stdout, &mut stderr);
    Out { code, stdout, stderr }
}

/// 寫到第 limit byte 之後一定失敗的 writer：用來覆蓋 applet 的寫入錯誤分支。
pub struct FailWriter {
    pub written: usize,
    pub limit: usize,
    pub out: Vec<u8>,
}

impl FailWriter {
    pub fn new(limit: usize) -> FailWriter {
        FailWriter { written: 0, limit, out: Vec::new() }
    }
}

impl std::io::Write for FailWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        if self.written >= self.limit {
            return Err(std::io::Error::new(std::io::ErrorKind::BrokenPipe, "writer failed"));
        }
        let n = buf.len().min(self.limit - self.written);
        self.out.extend_from_slice(&buf[..n]);
        self.written += n;
        Ok(n)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// 用自訂 writer 跑 applet（覆蓋寫入錯誤分支）。
pub fn run_with(name: &str, args: &[&str], input: &[u8], stdout: &mut dyn std::io::Write, stderr: &mut dyn std::io::Write) -> i32 {
    let mut stdin = input;
    let v: Vec<String> = args.iter().map(|s| s.to_string()).collect();
    dispatch(name, &v, &mut stdin, stdout, stderr)
}

/// e2e：跑真正的 binary。binary 路徑由 COREUTILS_BIN 給（guest 內是 /mnt/bin/coreutils）。
pub fn run_bin(name: &str, args: &[&str], input: &str) -> Out {
    let bin = std::env::var("COREUTILS_BIN").unwrap_or_else(|_| "coreutils".into());
    let mut cmd = std::process::Command::new(&bin);
    if std::env::var("COREUTILS_BIN").is_ok() {
        // multi-call 模式：argv[0] 是 binary 本身時，applet 名稱來自第一個參數
        cmd.arg(name);
    }
    cmd.args(args);
    let out = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .and_then(|mut p| {
            use std::io::Write;
            p.stdin
                .as_mut()
                .ok_or(std::io::Error::new(std::io::ErrorKind::Other, "no stdin"))?
                .write_all(input.as_bytes())?;
            p.wait_with_output()
        })
        .expect("failed to run binary");
    Out {
        code: out.status.code().unwrap_or(-1),
        stdout: out.stdout,
        stderr: out.stderr,
    }
}

/// e2e 但限制 stdout 讀取量：用來測會無限輸出的 applet（yes）。
/// 讀到 limit byte 後殺掉子行程；code 是 -1（被 kill），呼叫端只該看 stdout。
pub fn run_bin_limit(name: &str, args: &[&str], limit: usize) -> Out {
    let bin = std::env::var("COREUTILS_BIN").unwrap_or_else(|_| "coreutils".into());
    let mut cmd = std::process::Command::new(&bin);
    if std::env::var("COREUTILS_BIN").is_ok() {
        cmd.arg(name);
    }
    cmd.args(args);
    let mut p = cmd
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .expect("failed to run binary");
    let mut stdout = Vec::new();
    let mut buf = [0u8; 4096];
    if let Some(rd) = p.stdout.as_mut() {
        while stdout.len() < limit {
            match rd.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => stdout.extend_from_slice(&buf[..n.min(limit - stdout.len())]),
                Err(_) => break,
            }
        }
    }
    let _ = p.kill();
    let _ = p.wait();
    Out {
        code: -1,
        stdout,
        stderr: Vec::new(),
    }
}

/// guest 內可寫的工作目錄（initramfs 的 root 是 ramfs，/tmp 可寫）。
/// 9p share 是 read-only，所以測試產生的檔案一律放在這裡。
pub fn workdir() -> std::path::PathBuf {
    let d = std::env::var("COREUTILS_WORK").unwrap_or_else(|_| "/tmp".into());
    std::fs::create_dir_all(&d).ok();
    std::path::PathBuf::from(d)
}

/// 確定性 PRNG（xorshift64*）：fuzz 可重現，不依賴外部 crate。
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Rng {
        Rng(if seed == 0 { 0x9e3779b97f4a7c15 } else { seed })
    }
    pub fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }
    pub fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            0
        } else {
            self.next() % n
        }
    }
    /// 隨機 byte 流（避開 NUL，讓文字類 applet 的行為可觀察）
    pub fn bytes(&mut self, len: usize) -> Vec<u8> {
        let mut v = Vec::with_capacity(len);
        for _ in 0..len {
            let b = (self.next() & 0xFF) as u8;
            v.push(if b == 0 { 0x20 } else { b });
        }
        v
    }
}
