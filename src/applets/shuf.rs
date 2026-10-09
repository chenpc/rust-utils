//! shuf：依 man page（docs/man/shuf.1.gz）實作。
//!
//! 輸入來源：`-e`（每個 ARG 是一行）、`-i LO-HI`、或檔案/stdin。
//! 輸出：隨機排列；`-n COUNT` 最多 COUNT 行；`-r` 可重複。
//! `--random-seed STRING` 讓輸出可重現（本實作用 FNV-1a 種子 + xorshift64*）。

use crate::In;
use crate::Out;

struct Rng {
    state: u64,
}

impl Rng {
    fn new(seed: &str) -> Rng {
        let mut h: u64 = 0xcbf29ce484222325;
        for b in seed.as_bytes() {
            h ^= *b as u64;
            h = h.wrapping_mul(0x100000001b3);
        }
        if h == 0 {
            h = 0x9e3779b97f4a7c15;
        }
        Rng { state: h }
    }

    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F4914F6CDD1D)
    }

    fn below(&mut self, n: u64) -> u64 {
        if n == 0 {
            return 0;
        }
        self.next() % n
    }
}

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut echo = false;
    let mut range: Option<(i64, i64)> = None;
    let mut count: Option<usize> = None;
    let mut output = String::new();
    let mut seed: Option<String> = None;
    let mut repeat = false;
    let mut term = b'\n';
    let mut files: Vec<String> = Vec::new();

    let mut i = 0;
    while i < args.len() {
        let a = args[i].as_str();
        match a {
            "-e" | "--echo" => echo = true,
            "-r" | "--repeat" => repeat = true,
            "-z" | "--zero-terminated" => term = 0,
            "-i" | "--input-range" => {
                if i + 1 >= args.len() {
                    let _ = stderr.write_all(b"shuf: missing operand for -i\n");
                    return 1;
                }
                let spec = args[i + 1].clone();
                i += 1;
                let Some((lo, hi)) = spec.split_once('-') else {
                    let _ = stderr.write_all(b"shuf: invalid input range\n");
                    return 1;
                };
                let lo = lo.parse::<i64>();
                let hi = hi.parse::<i64>();
                if lo.is_err() || hi.is_err() {
                    let _ = stderr.write_all(b"shuf: invalid input range\n");
                    return 1;
                }
                range = Some((lo.unwrap(), hi.unwrap()));
            }
            "-n" | "--head-count" => {
                if i + 1 >= args.len() {
                    let _ = stderr.write_all(b"shuf: missing operand for -n\n");
                    return 1;
                }
                match args[i + 1].parse::<usize>() {
                    Ok(v) => count = Some(v),
                    Err(_) => {
                        let _ = stderr.write_all(b"shuf: invalid head count\n");
                        return 1;
                    }
                }
                i += 1;
            }
            "-o" | "--output" => {
                if i + 1 >= args.len() {
                    let _ = stderr.write_all(b"shuf: missing operand for -o\n");
                    return 1;
                }
                output = args[i + 1].clone();
                i += 1;
            }
            "--random-seed" => {
                if i + 1 >= args.len() {
                    let _ = stderr.write_all(b"shuf: missing operand for --random-seed\n");
                    return 1;
                }
                seed = Some(args[i + 1].clone());
                i += 1;
            }
            "--random-source" => {
                if i + 1 < args.len() {
                    i += 1;
                }
            }
            "-h" | "--help" => {
                if stdout.write_all(b"Usage: shuf [-e] [-i LO-HI] [-n COUNT] [-o FILE] [-r] [-z] [FILE]\n").is_err() {
                    return 1;
                }
                return 0;
            }
            "-V" | "--version" => {
                if stdout.write_all(b"shuf (uutils coreutils) 0.10.0\n").is_err() {
                    return 1;
                }
                return 0;
            }
            _ => files.push(a.to_string()),
        }
        i += 1;
    }

    let mut lines: Vec<String> = Vec::new();
    if echo {
        lines = files.clone();
    } else if let Some((lo, hi)) = range {
        for n in lo..=hi {
            lines.push(n.to_string());
        }
    } else if files.is_empty() {
        let mut buf = Vec::new();
        if stdin.read_to_end(&mut buf).is_err() {
            return 1;
        }
        for chunk in buf.split(|c| *c == term) {
            if !chunk.is_empty() {
                lines.push(String::from_utf8_lossy(chunk).to_string());
            }
        }
    } else {
        for f in &files {
            match std::fs::read(f) {
                Ok(b) => {
                    for chunk in b.split(|c| *c == term) {
                        if !chunk.is_empty() {
                            lines.push(String::from_utf8_lossy(chunk).to_string());
                        }
                    }
                }
                Err(e) => {
                    let msg = format!("shuf: {f}: {e}\n");
                    if stderr.write_all(msg.as_bytes()).is_err() {
                        return 1;
                    }
                    return 1;
                }
            }
        }
    }

    if lines.is_empty() {
        return 0;
    }

    let seed_str = seed.unwrap_or_else(|| "0".to_string());
    let mut rng = Rng::new(&seed_str);

    let picked: Vec<String> = if repeat {
        let n = count.unwrap_or(lines.len());
        (0..n)
            .map(|_| lines[rng.below(lines.len() as u64) as usize].clone())
            .collect()
    } else {
        let mut idx: Vec<usize> = (0..lines.len()).collect();
        for k in 0..idx.len() {
            let j = k + rng.below((idx.len() - k) as u64) as usize;
            idx.swap(k, j);
        }
        idx.truncate(count.unwrap_or(idx.len()));
        idx.iter().map(|&k| lines[k].clone()).collect()
    };

    let text: String = picked.iter().map(|l| format!("{l}{}", term as char)).collect();
    if output.is_empty() {
        if stdout.write_all(text.as_bytes()).is_err() {
            return 1;
        }
        return 0;
    }
    match std::fs::write(&output, text.as_bytes()) {
        Ok(()) => 0,
        Err(e) => {
            let msg = format!("shuf: {output}: {e}\n");
            if stderr.write_all(msg.as_bytes()).is_err() {
                return 1;
            }
            1
        }
    }
}
