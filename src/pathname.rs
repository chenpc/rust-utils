//! 路徑/名稱家族共用核心：basename、dirname、pathchk、realpath、readlink
//!
//! 行為模型來源（objective：man page 為主，GNU 與 uutils 衝突時以 GNU 為準）：
//! - 本機 man mirror：`docs/man/{basename,dirname,pathchk,realpath,readlink}.1.gz`
//! - GNU coreutils 9.5 info 節點文字（`docs/gnu_info/group_c_realpath_family.txt`）
//! - 落差記錄：`docs/assumptions.md`
//!
//! 不參照任何 coreutils source code。


/// POSIX 最小可移植上限（`pathchk -p`）。數值取自 POSIX 標準名稱，
/// man page 只寫「POSIX minimum limits for portability」（假設見 docs/assumptions.md）。
pub const POSIX_PATH_MAX: usize = 256;
pub const POSIX_NAME_MAX: usize = 14;
/// 本機檔案系統上限（`pathchk` 預設檢查）：Linux PATH_MAX。
pub const OS_PATH_MAX: usize = 4096;

/// 存在性要求。
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// 除最後一個 component 外都必須存在（realpath 預設、readlink -f）。
    #[default]
    Default,
    /// 所有 component 都必須存在（-e）。
    Existing,
    /// 不要求存在，缺失的 component 視為目錄（-m）。
    Missing,
}

#[derive(Clone, Copy, Default)]
pub struct Canon<'a> {
    pub mode: Mode,
    /// -s / --strip / --no-symlinks：不解析 symlink，只做詞法化。
    pub strip: bool,
    /// -L：先處理 `..` 再解析 symlink（預設是 -P）。
    pub logical: bool,
    pub relative_to: Option<&'a str>,
    pub relative_base: Option<&'a str>,
}

/// 去掉尾端的 `/`（全為 `/` 時得到空字串）。
pub fn strip_trailing_slashes(s: &str) -> &str {
    let bytes = s.as_bytes();
    let mut end = bytes.len();
    while end > 0 && bytes[end - 1] == b'/' {
        end -= 1;
    }
    &s[..end]
}

/// 最後一個 component（不含尾端 `/`）。全為 `/` 時回傳空字串。
pub fn base_part(name: &str) -> &str {
    let t = strip_trailing_slashes(name);
    match t.rfind('/') {
        Some(i) => &t[i + 1..],
        None => t,
    }
}

/// 除最後一個 component 以外的部分。
pub fn dir_part(name: &str) -> String {
    if name.is_empty() {
        return ".".to_string();
    }
    let t = strip_trailing_slashes(name);
    if t.is_empty() {
        // NAME 全是 `/`：Linux 上下傳 `/`。
        return "/".to_string();
    }
    let i = match t.rfind('/') {
        Some(i) => i,
        None => return ".".to_string(),
    };
    let d = strip_trailing_slashes(&t[..i]);
    if d.is_empty() {
        "/".to_string()
    } else {
        d.to_string()
    }
}

/// 若 `base` 以 `suffix` 結尾且兩者不相同，移除 suffix。
pub fn strip_suffix<'a>(base: &'a str, suffix: &str) -> &'a str {
    if !suffix.is_empty() && base != suffix && base.ends_with(suffix) {
        &base[..base.len() - suffix.len()]
    } else {
        base
    }
}

/// 以 `/` 切 component，略過空段。
pub fn components(s: &str) -> Vec<&str> {
    s.split('/').filter(|c| !c.is_empty()).collect()
}

/// POSIX 可移植字元集：大小寫字母、數字、`.`、`_`、`-`、`/`。
pub fn portable_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/')
}

#[derive(Clone, Copy, Default)]
pub struct CheckOpts {
    /// -p：改用 POSIX 可移植性檢查。
    pub posix: bool,
    /// -P：空名稱或 component 以 `-` 開頭。
    pub leading_hyphen: bool,
}

/// 回傳所有錯誤原因（空代表通過）。呼叫端負責輸出 `<applet>: <name>: <reason>`。
pub fn check_name(name: &str, o: CheckOpts) -> Vec<String> {
    let mut errs: Vec<String> = Vec::new();
    let comps = components(name);
    // 參考 binary 的訊息用單引號包住名稱 / component。
    let q = |s: &str| format!("'{s}'");

    if o.posix {
        if name.is_empty() {
            errs.push("empty file name".to_string());
        }
        for c in name.chars() {
            if !portable_char(c) {
                // 找出該字元所屬的 component。
                let comp = name
                    .split('/')
                    .find(|seg| seg.contains(c))
                    .unwrap_or(name);
                errs.push(format!(
                    "nonportable character '{}' in file name component {}",
                    c,
                    q(comp)
                ));
                break;
            }
        }
        // 整體名稱已超限時不再回報 component 長度錯誤（與參考 binary 一致）。
        if name.len() > POSIX_PATH_MAX {
            // 參考 binary 在 POSIX 模式不加引號（與預設模式不同），照抄以維持可重現性。
            errs.push(format!(
                "limit {} exceeded by length {} of file name {}",
                POSIX_PATH_MAX,
                name.len(),
                name
            ));
        } else {
            for c in &comps {
                if c.len() > POSIX_NAME_MAX {
                    errs.push(format!(
                        "limit {} exceeded by length {} of file name component {}",
                        POSIX_NAME_MAX,
                        c.len(),
                        q(c)
                    ));
                    break; // 只報第一個超限 component（與參考 binary 一致）
                }
            }
        }
    } else if name.is_empty() {
        // 預設模式：空名稱 → 特殊訊息（參考 binary 帶 applet 前綴與引號）。
        errs.push(format!("pathchk: {}: No such file or directory", q(name)));
    } else if name.len() > OS_PATH_MAX {
        errs.push(format!(
            "limit {} exceeded by length {} of file name {}",
            OS_PATH_MAX,
            name.len(),
            q(name)
        ));
    }

    if o.leading_hyphen {
        for c in &comps {
            if c.starts_with('-') {
                errs.push(format!("leading hyphen in file name component {}", q(c)));
            }
        }
    }

    // 檔案系統檢查：沿 component 前綴 stat；前面的檢查有錯就跳過。
    // ENOENT / ELOOP 被容忍（名稱不存在不是 pathchk 的錯誤）；其他錯誤如實回報。
    if errs.is_empty() && !name.is_empty() {
        let mut cur = String::new();
        for c in &comps {
            if !cur.is_empty() {
                cur.push('/');
            }
            cur.push_str(c);
            let path = if name.starts_with('/') {
                format!("/{cur}")
            } else {
                cur.clone()
            };
            if let Err(e) = std::fs::metadata(&path) {
                match e.raw_os_error() {
                    Some(2) | Some(40) => break, // ENOENT / ELOOP → 容忍
                    _ => {
                        errs.push(e.to_string()); // e.g. "Permission denied (os error 13)"
                        break;
                    }
                }
            }
        }
    }
    errs
}

fn build_path_owned(comps: &[String]) -> String {
    if comps.is_empty() {
        return "/".to_string();
    }
    let mut s = String::new();
    for c in comps {
        s.push('/');
        s.push_str(c);
    }
    s
}

/// 同 io_reason_pub 但去掉 "(os error N)" 後綴（mkdir/rmdir/truncate 用）
pub fn io_reason_nosuffix(e: &std::io::Error) -> String {
    io_reason_pub(e).split(" (os error ").next().unwrap_or("").to_string()
}

pub fn io_reason_pub(e: &std::io::Error) -> String {
    match e.raw_os_error() {
        Some(2) => "No such file or directory".to_string(),  // ENOENT

        Some(13) => "Permission denied".to_string(),
        Some(20) => "Not a directory".to_string(),
        Some(36) => "File name too long".to_string(),
        Some(40) => "Too many levels of symbolic links".to_string(),
        Some(14) => "Bad address".to_string(),
        _ => e.to_string(),
    }
}

fn io_reason(e: &std::io::Error) -> String {
    io_reason_pub(e)
}

/// multi-call 時參考用 `<argv0 dirname>/<applet>`；symlink 直呼時 argv[0] 已是 /bin/<applet>。
pub fn invoked_as(applet: &str) -> String {
    let mut argv = std::env::args();
    let a0 = argv.next().unwrap_or_default();
    let a1 = argv.next().unwrap_or_default();
    if a1 == applet {
        let dir = a0.rsplit_once('/').map(|(d, _)| d).unwrap_or("");
        if dir.is_empty() {
            applet.to_string()
        } else {
            format!("{}/{}", dir, applet)
        }
    } else {
        a0
    }
}

/// clap 風格「短旗標簇中未知字元」的顯示規則（實測）：
/// `-sq` → `-q`、`-3x` → `-3`、`-x0` → `-x`；字元 `0` 一律顯示成 `0`（不帶 dash）。
pub fn clap_unknown_short(token: &str, k: usize) -> String {
    if token.as_bytes()[k] == b'0' {
        "0".to_string()
    } else {
        format!("-{}", &token[k..=k])
    }
}

/// uutils 的 usage hint：`Try '<path>/<applet> --help' for more information.`
pub fn usage_hint(applet: &str) -> String {
    format!("Try '{} --help' for more information.", invoked_as(applet))
}

/// clap 風格「選項缺值」錯誤（uutils 對 clap long option 的實測措辭）：
/// `error: a value is required for '--mode <MODE>' but none was supplied` + 空行 +
/// `For more information, try '--help'.`
pub fn clap_value_required(long: &str, metavar: &str, stderr: crate::Out) {
    if long.len() == 1 {
        writeln!(
            stderr,
            "error: a value is required for '-{} <{}>' but none was supplied",
            long, metavar
        )
        .ok();
    } else {
        writeln!(
            stderr,
            "error: a value is required for '--{} <{}>' but none was supplied",
            long, metavar
        )
        .ok();
    }
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
}

/// clap 風格「選項重複」錯誤（實測：`pwd -L -L`、`printenv -0 -0`、`env -i -i`）。
/// 措辭 `'--<long>[ <metavar>]'`，只有 error 行 + 空行 + `For more information, try '--help'.`
/// （沒有 tip、沒有 Usage 段）。呼叫端決定 exit code（pwd 1、printenv 2、env 125）。
pub fn clap_dup(long: &str, metavar: &str, code: i32, stderr: crate::Out) -> i32 {
    if metavar.is_empty() {
        writeln!(
            stderr,
            "error: the argument '--{}' cannot be used multiple times",
            long
        )
        .ok();
    } else {
        writeln!(
            stderr,
            "error: the argument '--{} <{}>' cannot be used multiple times",
            long, metavar
        )
        .ok();
    }
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    code
}

/// clap `infer_long_args` 語意：長選項可用「唯一前綴」指定。
/// 實測（參考 binary）：`mktemp --q` = `--quiet`、`mktemp --tmpdi` = `--tmpdir`、
/// `shred --verb` = `--verbose`、`mknod --hel` = `--help`；
/// 歧義（`shred --v` 對 `--verbose`/`--version`）與無匹配一律回傳 None，
/// 由各 applet 走 unknown-option 錯誤路徑（clap 在歧義時还会给模糊 tip，本專案未重現）。
pub fn infer_long_opt<'a>(name: &str, longs: &[&'a str]) -> Option<&'a str> {
    if name.is_empty() {
        return None;
    }
    if let Some(exact) = longs.iter().find(|l| **l == name) {
        return Some(exact);
    }
    let mut hits = longs.iter().filter(|l| l.starts_with(name));
    let first = hits.next()?;
    if hits.next().is_some() {
        return None;
    }
    Some(first)
}

/// 詞法化：處理 `.`、`..`、多餘 `/`，不碰檔案系統。
/// 相對路徑開頭的 `..` 會保留（無法在詞法層去掉）。
pub fn lexical(absolute: bool, comps: &[&str]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for c in comps {
        match *c {
            "." => {}
            ".." => {
                if let Some(last) = out.last() {
                    if last == ".." {
                        out.push("..".to_string());
                    } else {
                        out.pop();
                    }
                } else if !absolute {
                    out.push("..".to_string());
                }
            }
            other => out.push(other.to_string()),
        }
    }
    out
}

/// 沿路解析 symlink（物理模式：symlink 先解析，才處理其後的 `..`）。
fn walk_physical(name: &str, mode: Mode) -> Result<String, String> {
    let absolute = name.starts_with('/');
    let trailing = name.ends_with('/');
    let mut cur: Vec<String> = if absolute {
        Vec::new()
    } else {
        let cwd = std::env::current_dir().map_err(|e| io_reason(&e))?;
        components(&cwd.to_string_lossy()).iter().map(|c| c.to_string()).collect()
    };
    let mut pending: Vec<String> = components(name).iter().map(|c| c.to_string()).collect();
    let mut links = 0usize;
    let mut i = 0usize;
    while i < pending.len() {
        let c = pending[i].clone();
        let last = i + 1 == pending.len();
        i += 1;
        if c == "." {
            continue;
        }
        if c == ".." {
            if !cur.is_empty() {
                cur.pop();
            } else if !absolute {
                cur.push("..".to_string());
            }
            continue;
        }
        let mut trial = cur.clone();
        trial.push(c.clone());
        let cand = build_path_owned(&trial);
        match std::fs::symlink_metadata(&cand) {
            Ok(md) if md.file_type().is_symlink() => {
                links += 1;
                if links > 100 {
                    return Err("Too many levels of symbolic links".to_string());
                }
                let target = std::fs::read_link(&cand).map_err(|e| io_reason(&e))?;
                let t = target.to_string_lossy().to_string();
                let mut head: Vec<String> = components(&t).iter().map(|x| x.to_string()).collect();
                if t.starts_with('/') {
                    cur.clear();
                }
                head.extend_from_slice(&pending[i..]);
                pending = head;
                i = 0;
            }
            Ok(_) => {
                cur = trial;
            }
            Err(e) => {
                if mode == Mode::Missing {
                    cur = trial;
                } else if last && mode == Mode::Default {
                    cur = trial;
                } else {
                    return Err(io_reason(&e));
                }
            }
        }
    }
    let _ = trailing; // 參考 binary 不因尾端 `/` 報錯
    Ok(build_path_owned(&cur))
}

/// 邏輯模式（-L）：先把 `..` 詞法化，再解析 symlink。
fn walk_logical(name: &str, mode: Mode) -> Result<String, String> {
    let absolute = name.starts_with('/');
    let trailing = name.ends_with('/');
    let base: Vec<String> = if absolute {
        Vec::new()
    } else {
        let cwd = std::env::current_dir().map_err(|e| io_reason(&e))?;
        components(&cwd.to_string_lossy()).iter().map(|c| c.to_string()).collect()
    };
    let mut merged: Vec<String> = base;
    for c in components(name) {
        match c {
            "." => {}
            ".." => {
                if !merged.is_empty() {
                    merged.pop();
                } else if !absolute {
                    merged.push("..".to_string());
                }
            }
            other => merged.push(other.to_string()),
        }
    }
    let lexed = build_path_owned(&merged);
    // 此時已無 `.`/`..`，只需逐 component 解析 symlink。
    let mut cur: Vec<String> = Vec::new();
    let mut pending: Vec<String> = components(&lexed).iter().map(|c| c.to_string()).collect();
    let mut links = 0usize;
    let mut i = 0usize;
    while i < pending.len() {
        let c = pending[i].clone();
        let last = i + 1 == pending.len();
        i += 1;
        let mut trial = cur.clone();
        trial.push(c.clone());
        let cand = build_path_owned(&trial);
        match std::fs::symlink_metadata(&cand) {
            Ok(md) if md.file_type().is_symlink() => {
                links += 1;
                if links > 100 {
                    return Err("Too many levels of symbolic links".to_string());
                }
                let target = std::fs::read_link(&cand).map_err(|e| io_reason(&e))?;
                let t = target.to_string_lossy().to_string();
                let mut head: Vec<String> = components(&t).iter().map(|x| x.to_string()).collect();
                if t.starts_with('/') {
                    cur.clear();
                }
                head.extend_from_slice(&pending[i..]);
                pending = head;
                i = 0;
            }
            Ok(_) => cur = trial,
            Err(e) => {
                if mode == Mode::Missing || (last && mode == Mode::Default) {
                    cur = trial;
                } else {
                    return Err(io_reason(&e));
                }
            }
        }
    }
    let _ = trailing; // 參考 binary 不因尾端 `/` 報錯
    Ok(build_path_owned(&cur))
}

/// 只做詞法化（-s）；存在性要求依 mode（-m 時完全不碰檔案）。
fn walk_strip(name: &str, mode: Mode) -> Result<String, String> {
    let absolute = name.starts_with('/');
    let base: Vec<String> = if absolute {
        Vec::new()
    } else {
        let cwd = std::env::current_dir().map_err(|e| io_reason(&e))?;
        components(&cwd.to_string_lossy()).iter().map(|c| c.to_string()).collect()
    };
    let mut merged: Vec<String> = base;
    for c in components(name) {
        match c {
            "." => {}
            ".." => {
                if !merged.is_empty() {
                    merged.pop();
                } else if !absolute {
                    merged.push("..".to_string());
                }
            }
            other => merged.push(other.to_string()),
        }
    }
    let out = build_path_owned(&merged);
    if mode != Mode::Missing {
        match std::fs::metadata(&out) {
            Ok(_) => {}
            Err(e) => {
                if mode == Mode::Existing {
                    return Err(io_reason(&e));
                }
                // Default：最後一段允許不存在（前面的段由呼叫端的名字結構保證）。
                let _ = e;
            }
        }
    }
    Ok(out)
}

/// 解析一個檔案名稱；回傳絕對（或 -s 時的詞法）路徑。
pub fn canonicalize(name: &str, o: &Canon) -> Result<String, String> {
    if o.strip {
        return walk_strip(name, o.mode);
    }
    if o.logical {
        return walk_logical(name, o.mode);
    }
    walk_physical(name, o.mode)
}

/// 詞法化 + 組回絕對路徑（relpath 用：假設輸入已是絕對路徑）。
pub fn build_lexical_absolute(comps: &[&str]) -> String {
    let lexed = lexical(true, comps);
    build_path_owned(&lexed)
}

/// `target` 是否為 `base` 的子孫（含相等）。兩者都已是絕對路徑。
pub fn is_descendant(base: &str, target: &str) -> bool {
    target == base || target.starts_with(&format!("{}/", base.trim_end_matches('/')))
}

/// 印出 `target` 相對於 `base` 的路徑。
pub fn relative_from(base: &str, target: &str) -> String {
    let b = components(base);
    let t = components(target);
    let mut common = 0usize;
    while common < b.len() && common < t.len() && b[common] == t[common] {
        common += 1;
    }
    let mut out: Vec<String> = Vec::new();
    for _ in common..b.len() {
        out.push("..".to_string());
    }
    for c in &t[common..] {
        out.push(c.to_string());
    }
    if out.is_empty() {
        ".".to_string()
    } else {
        out.join("/")
    }
}

/// 依 --relative-to / --relative-base 決定要印相對路徑還是絕對路徑。
pub fn apply_relative(abs: &str, o: &Canon) -> String {
    if let Some(rel_base) = o.relative_base {
        // DIR1（relative-to）必須是 DIR2（relative-base）的子目錄才會印相對路徑。
        match o.relative_to {
            Some(rt) => {
                if is_descendant(rel_base, rt) && is_descendant(rt, abs) {
                    relative_from(rt, abs)
                } else {
                    abs.to_string()
                }
            }
            None => {
                if is_descendant(rel_base, abs) {
                    relative_from(rel_base, abs)
                } else {
                    abs.to_string()
                }
            }
        }
    } else if let Some(rt) = o.relative_to {
        relative_from(rt, abs)
    } else {
        abs.to_string()
    }
}

/// realpath/readlink 的單檔處理：回傳要印出的字串（None = 不印）。
pub fn resolve_one(name: &str, o: &Canon) -> Result<String, String> {
    let abs = canonicalize(name, o)?;
    if o.relative_to.is_some() || o.relative_base.is_some() {
        let base_abs = match o.relative_to {
            Some(rt) => Some(canonicalize(rt, &Canon { mode: Mode::Default, ..Canon::default() })?),
            None => None,
        };
        let base_rel = match o.relative_base {
            Some(rb) => Some(canonicalize(rb, &Canon { mode: Mode::Default, ..Canon::default() })?),
            None => None,
        };
        let eff = Canon {
            relative_to: base_abs.as_deref(),
            relative_base: base_rel.as_deref(),
            ..*o
        };
        return Ok(apply_relative(&abs, &eff));
    }
    Ok(abs)
}

/// clap `infer_long_args` 細部語意：
/// `Ok(long)` = 精確或唯一前綴；`Err(Some(cand))` = 前綴 ambiguous（clap 改用
/// `tip: a similar argument exists: '--<cand>'`）；`Err(None)` = 完全不符。
/// 注：clap 對「非前綴但編輯距離很近」的長旗標（如 `--silnt`、`--userr`）也會給 similar tip，
/// 但其選取規則與 clap 版本/applet 旗標清單相關、實測不一致（`--bog`→給 `--help` 的 tip、
/// `--bogus` 卻不給）；本專案不重現這條 fuzzy tip，記錄於 docs/assumptions.md。
pub fn resolve_long<'a>(name: &str, longs: &'a [&'a str]) -> Result<&'a str, Option<&'a str>> {
    if name.is_empty() {
        return Err(None);
    }
    for l in longs {
        if *l == name {
            return Ok(l);
        }
    }
    let candidates: Vec<&'a str> = longs.iter().filter(|l| l.starts_with(name)).copied().collect();
    match candidates.len() {
        0 => Err(None),
        1 => Ok(candidates[0]),
        _ => Err(Some(candidates[0])),
    }
}

/// clap 對「不需要值的旗標卻給了值」的措辭。
pub fn clap_unexpected_value(
    long: &str,
    value: &str,
    usage: &str,
    stderr: crate::Out,
) -> i32 {
    writeln!(
        stderr,
        "error: unexpected value '{}' for '--{}' found; no more were expected",
        value, long
    )
    .ok();
    writeln!(stderr).ok();
    writeln!(stderr, "Usage: {}", usage).ok();
    writeln!(stderr).ok();
    writeln!(stderr, "For more information, try '--help'.").ok();
    1
}
