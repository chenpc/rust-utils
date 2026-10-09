use coreutils::hashsum as h;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

fn main() {
    for s in ["abc", "hello world\n", ""] {
        let d = s.as_bytes();
        println!("{} sha384 = {}", s.escape_debug(), hex(&h::digest(&h::SHA384, d, 0)));
        println!("   sha512 = {}", hex(&h::digest(&h::SHA512, d, 0)));
    }
}
