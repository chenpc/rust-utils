use crate::hashsum as h;

pub fn run(args: &[String], stdin: &mut dyn std::io::Read, stdout: &mut dyn std::io::Write, stderr: &mut dyn std::io::Write) -> i32 {
    h::run(&h::SHA3_224, "sha3-224sum", args, stdin, stdout, stderr, false)
}
