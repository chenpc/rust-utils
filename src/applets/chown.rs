// chown：使用 chown_common 共用核心（見 src/chown_common.rs）。


pub fn run(args: &[String], _stdin: crate::In, stdout: crate::Out, stderr: crate::Out) -> i32 {
    let parsed = match crate::chown_common::parse_common("chown", args, stdout, stderr) {
        Ok(p) => p,
        Err(rc) => return rc,
    };
    let spec = match parsed.spec {
        Some(s) => s,
        None => {
            if parsed.opts.reference.is_none() {
                writeln!(stderr, "chown: missing operand").ok();
                return 2;
            }
            String::new()
        }
    };
    if parsed.opts.reference.is_some() {
        // 參考 uutils：有 --reference 時所有 positional 均為 FILE
        if !spec.is_empty() {
            let mut files = parsed.files.clone();
            files.insert(0, spec.clone());
            return crate::chown_common::run("chown", &files, &parsed.opts, None, None, stdout, stderr);
        }
    }
    if parsed.files.is_empty() {
        writeln!(stderr, "chown: missing operand").ok();
        return 2;
    }
    let (uid, gid) = match crate::chown_common::parse_owner(&spec, true, false, "chown", stderr) {
        Some(v) => v,
        None => return 1,
    };
    crate::chown_common::run("chown", &parsed.files, &parsed.opts, uid, gid, stdout, stderr)
}
