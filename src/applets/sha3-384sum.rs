use crate::hashsum as h;

pub fn run(args: &[String], stdin: &mut dyn std::io::Read, stdout: &mut dyn std::io::Write, stderr: &mut dyn std::io::Write) -> i32 {
    h::run(&h::SHA3_384, "sha3-384sum", args, stdin, stdout, stderr, false)
}
