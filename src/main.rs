//! multi-call binary 的進入點：applet 由 argv[0] 決定。
//!
//! argv[0] 是 `coreutils` 時走 multi-call 模式（applet 名稱是第一個參數）；
//! 否則 argv[0] 的 basename 就是 applet 名稱（symlink 到 coreutils 的那種用法）。

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    if argv.is_empty() {
        std::process::exit(2);
    }

    let base = argv[0].rsplit('/').next().unwrap_or(&argv[0]).to_string();
    let (applet, args): (String, Vec<String>) = if base == "coreutils" {
        if argv.len() < 2 {
            eprintln!("coreutils: no applet given");
            std::process::exit(2);
        }
        (argv[1].clone(), argv[2..].to_vec())
    } else {
        (base, argv[1..].to_vec())
    };

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let stderr = std::io::stderr();
    let mut r = stdin.lock();
    let mut w = stdout.lock();
    let mut e = stderr.lock();

    std::process::exit(coreutils::dispatch(&applet, &args, &mut r, &mut w, &mut e));
}
