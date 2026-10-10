#!/usr/bin/env python3
"""依 src/applets/ 目錄產生 src/applets/mod.rs 與 src/lib.rs 的 lookup 表。

避免 115 個 applet 各自手動註冊；applet 檔案本身就是註冊來源。
Rust 關鍵字（true/false）用 raw identifier `r#true`，檔案名仍是 true.rs。
"""
import pathlib
import re

KEYWORDS = {"false", "true", "fn", "if", "match", "loop", "type", "struct", "move", "as", "in", "box", "crate", "enum", "impl", "mod", "pub", "ref", "static", "trait", "use", "while"}

root = pathlib.Path(__file__).resolve().parent.parent
apps = root / "src" / "applets"
names = sorted(p.stem for p in apps.glob("*.rs") if p.stem != "mod")

def ident(n):
    if n in KEYWORDS:
        return f"r#{n}"
    if not n.isidentifier():
        return n.replace("-", "_")
    return n

mod_lines = ["//! applet 模組：每個 applet 一個檔案（src/applets/<name>.rs），行為依據是 man page。", ""]
for n in names:
    idn = ident(n)
    if idn != n:
        mod_lines.append(f'#[path = "{n}.rs"] pub mod {idn};')
    else:
        mod_lines.append(f"pub mod {idn};")
apps.joinpath("mod.rs").write_text("\n".join(mod_lines) + "\n")

# 一個檔案可以對應多個 applet 名稱（例如 test.rs 同時提供 test 與 [）
ALIASES = {"test": {"test": "run", "[": "run_bracket"}, "ls": {"ls": "run_ls", "dir": "run_dir", "vdir": "run_vdir"}}

arms = []
for n in names:
    for applet, func in ALIASES.get(n, {n: "run"}).items():
        arms.append(f'        "{applet}" => Some(applets::{ident(n)}::{func}),')
arms = "\n".join(sorted(arms))
lib = root / "src" / "lib.rs"
s = lib.read_text()
new = re.sub(r'    match name \{.*?\n    \}', "    match name {\n" + arms + "\n        _ => None,\n    }", s, count=1, flags=re.S)
lib.write_text(new)
print(f"registered {len(names)} applets")
