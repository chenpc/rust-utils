//! tsort：依 man page（docs/man/tsort.1.gz）實作。
//!
//! 字串以空白（tab、space、newline）分隔；每行的第 1 個字串依賴於第 2 個（a b = a 先於 b）。
//! 輸出拓撲順序：依「首次出現順序」挑選入度 0 的節點，印出後移除它的邊。
//! 有循環時輸出 tsort: cycle in data 並結束 1。

use crate::In;
use crate::Out;

pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut file = String::new();
    for a in args {
        if a == "-h" || a == "--help" {
            if stdout.write_all(b"Usage: tsort [FILE]\n").is_err() {
                return 1;
            }
            return 0;
        }
        if a == "-V" || a == "--version" {
            if stdout.write_all(b"tsort (uutils coreutils) 0.10.0\n").is_err() {
                return 1;
            }
            return 0;
        }
        file = a.clone();
    }

    let data: Vec<u8> = if file.is_empty() {
        let mut buf = Vec::new();
        if stdin.read_to_end(&mut buf).is_err() {
            return 1;
        }
        buf
    } else {
        match std::fs::read(&file) {
            Ok(b) => b,
            Err(e) => {
                let msg = format!("tsort: {file}: {e}\n");
                if stderr.write_all(msg.as_bytes()).is_err() {
                    return 1;
                }
                return 1;
            }
        }
    };

    let tokens: Vec<String> = data
        .split(|c| *c == b' ' || *c == b'\t' || *c == b'\n' || *c == b'\r')
        .filter(|s| !s.is_empty())
        .map(|s| String::from_utf8_lossy(s).to_string())
        .collect();

    let mut names: Vec<String> = Vec::new();
    let mut index = std::collections::HashMap::<String, usize>::new();
    for t in &tokens {
        if !index.contains_key(t) {
            index.insert(t.clone(), names.len());
            names.push(t.clone());
        }
    }

    // 每行兩個字串：a → b（a 先於 b）
    let mut succ = vec![Vec::<usize>::new(); names.len()];
    let mut indeg = vec![0usize; names.len()];
    let mut i = 0;
    while i + 1 < tokens.len() {
        let a = index[&tokens[i]];
        let b = index[&tokens[i + 1]];
        succ[a].push(b);
        indeg[b] += 1;
        i += 2;
    }

    let mut printed = vec![false; names.len()];
    let mut count = 0;
    while count < names.len() {
        let mut pick = None;
        for k in 0..names.len() {
            if !printed[k] && indeg[k] == 0 {
                pick = Some(k);
                break;
            }
        }
        let Some(k) = pick else {
            if stderr.write_all(b"tsort: cycle in data\n").is_err() {
                return 1;
            }
            return 1;
        };
        printed[k] = true;
        count += 1;
        if stdout.write_all(names[k].as_bytes()).is_err() || stdout.write_all(&[b'\n']).is_err() {
            return 1;
        }
        for &b in &succ[k] {
            indeg[b] -= 1;
        }
    }
    0
}
