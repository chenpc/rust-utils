// basenc — encode/decode data with a selectable base-N encoding.
//
// Behavior model (docs/man/basenc.1.gz):
//   --base64 (default), --base64url, --base32, --base32hex, --base16,
//   --base2lsbf, --base2msbf, --z85, --base58;
//   -d/--decode, -i/--ignore-garbage, -w COLS (default 76, 0 disables), [FILE].
//
// Assumption (docs/assumptions.md): the encoding selector may appear anywhere in the
// arguments; the last one wins.

use crate::basen as b;

pub fn run(args: &[String], stdin: &mut dyn std::io::Read, stdout: &mut dyn std::io::Write, stderr: &mut dyn std::io::Write) -> i32 {
    let mut codec = &b::BASE64;
    let rest: Vec<String> = args
        .iter()
        .filter(|a| {
            match a.as_str() {
                "--base64" => codec = &b::BASE64,
                "--base64url" => codec = &b::BASE64URL,
                "--base32" => codec = &b::BASE32,
                "--base32hex" => codec = &b::BASE32HEX,
                "--base16" => codec = &b::BASE16,
                "--base2lsbf" => codec = &b::BASE2LSBF,
                "--base2msbf" => codec = &b::BASE2MSBF,
                "--z85" => codec = &b::Z85,
                "--base58" => codec = &b::BASE58,
                _ => return true,
            }
            false
        })
        .cloned()
        .collect();
    b::run(codec, "basenc", &rest, stdin, stdout, stderr, 76)
}