use crate::basen as b;

pub fn run(args: &[String], stdin: &mut dyn std::io::Read, stdout: &mut dyn std::io::Write, stderr: &mut dyn std::io::Write) -> i32 {
    b::run(&b::BASE64, "base64", args, stdin, stdout, stderr, 76)
}
