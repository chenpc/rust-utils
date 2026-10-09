use std::fs::OpenOptions;
use std::io::Write;

use crate::In;
use crate::Out;

/// tee: copy stdin to each FILE and to stdout.
pub fn run(args: &[String], stdin: In, stdout: Out, stderr: Out) -> i32 {
    let mut append = false;
    let mut files: Vec<String> = Vec::new();
    let mut mode = "warn".to_string();

    let mut i = 0;
    while i < args.len() {
        let a = &args[i];
        if a == "--help" {
            writeln!(stderr, "Usage: tee [OPTIONS] [FILE]").unwrap();
            return 0;
        }
        if a == "--version" {
            writeln!(stderr, "tee (coreutils)").unwrap();
            return 0;
        }
        if a == "-h" {
            writeln!(stderr, "Usage: tee [OPTIONS] [FILE]").unwrap();
            return 0;
        }
        if a == "-V" {
            writeln!(stderr, "tee (coreutils)").unwrap();
            return 0;
        }
        if a == "-a" || a == "--append" {
            append = true;
            i += 1;
            continue;
        }
        if a == "-i" || a == "--ignore-interrupts" {
            i += 1;
            continue;
        }
        if a == "-p" {
            mode = "exit-nopipe".to_string();
            i += 1;
            continue;
        }
        if a == "--output-error" {
            if i + 1 >= args.len() {
                writeln!(stderr, "tee: --output-error requires a value").unwrap();
                return 2;
            }
            mode = args[i + 1].clone();
            i += 2;
            continue;
        }
        if a.starts_with('-') && a != "-" {
            writeln!(stderr, "tee: unknown option {a}").unwrap();
            return 2;
        }
        files.push(a.clone());
        i += 1;
    }

    let mut handles: Vec<(String, std::fs::File)> = Vec::new();
    for f in &files {
        match OpenOptions::new().write(true).create(true).append(append).truncate(!append).open(f) {
            Ok(fh) => handles.push((f.clone(), fh)),
            Err(e) => {
                writeln!(stderr, "tee: {f}: {e}").unwrap();
                return 1;
            }
        }
    }

    let mut status = 0;
    let mut buf = [0u8; 8192];
    loop {
        let n = match stdin.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(_) => return 1,
        };
        if stdout.write_all(&buf[..n]).is_err() {
            if mode == "exit" || mode == "exit-nopipe" {
                return 1;
            }
            writeln!(stderr, "tee: standard output: write error").unwrap();
            status = 1;
        }
        for (name, fh) in &mut handles {
            if fh.write_all(&buf[..n]).is_err() {
                if mode == "exit" || mode == "exit-nopipe" {
                    return 1;
                }
                writeln!(stderr, "tee: {name}: write error").unwrap();
                status = 1;
            }
        }
    }
    status
}
