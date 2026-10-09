// 驗證 hash 核心與 python hashlib / 參考實作一致。
use coreutils::hashsum as h;

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{:02x}", x)).collect()
}

const D: &[u8] = b"hello world\n";

#[test]
fn standard_algorithms() {
    assert_eq!(hex(&h::digest(&h::MD5, D, 0)), "6f5902ac237024bdd0c176cb93063dc4");
    assert_eq!(hex(&h::digest(&h::SHA1, D, 0)), "22596363b3de40b06f981fb85d82312e8c0ed511");
    assert_eq!(hex(&h::digest(&h::SHA224, D, 0)), "95041dd60ab08c0bf5636d50be85fe9790300f39eb84602858a9b430");
    assert_eq!(hex(&h::digest(&h::SHA256, D, 0)), "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447");
    assert_eq!(hex(&h::digest(&h::SHA384, D, 0)), "6b3b69ff0a404f28d75e98a066d3fc64fffd9940870cc68bece28545b9a75086b343d7a1366838083e4b8f3ca6fd3c80");
    assert_eq!(hex(&h::digest(&h::SHA512, D, 0)), "db3974a97f2407b7cae1ae637c0030687a11913274d578492558e39c16c017de84eacdc8c62fe34ee4e12b4b1428817f09b6a2760c3f8a664ceae94d2434a593");
    assert_eq!(hex(&h::digest(&h::BLAKE2B, D, 0)), "fec91c70284c72d0d4e3684788a90de9338a5b2f47f01fedbe203cafd68708718ae5672d10eca804a8121904047d40d1d6cf11e7a76419357a9469af41f22d01");
    assert_eq!(hex(&h::digest(&h::SHA3, D, 0)), "a8009a7a528d87778c356da3a55d964719e818666a04e4f960c9e2439e35f138");
    assert_eq!(hex(&h::digest(&h::SHA3, D, 224)), "7eda3e8d26f147821a258850956f9ed640fb0b3a8a04ae56a2f58a32");
    assert_eq!(hex(&h::digest(&h::SHAKE128, D, 256)), "37d6c4dad1d36a34dfefaab9407acadffdba35689a89a9287f84bfa55cc0af49");
    assert_eq!(hex(&h::digest(&h::SHAKE256, D, 512)), "4b7b2eafa0af610fce30bc6fdcdc44adb08999b1db43b366e62996d7a0f01d3e436095b3c964c73c0d85e9f6623f67f4e82cc4a6983d7e88de7514bacf0af8a1");
    assert_eq!(hex(&h::digest(&h::SM3, D, 0)), "4cc2036b86431b5d2685a04d289dfe140a36baa854b01cb39fcd6009638e4e7a");
    assert_eq!(hex(&h::digest(&h::BLAKE3, D, 0)), "dc5a4edb8240b018124052c330270696f96771a63b45250a5c17d3000e823355");
}

#[test]
fn sum_algorithms() {
    assert_eq!(h::sysv(D), 1126);
    assert_eq!(h::bsd(D), 3762);
    // crc：man page 沒定義 uutils 的 crc 演算法 → 本專案定義為標準 CRC-32
    // （非反射 poly 0x04c11db7、init 0、無最後 xor），與參考實作不同（docs/assumptions.md）。
    assert_eq!(h::crc32_msb(D, &h::crc_table_msb(0x04c11db7)), 3096396776);
    // crc32b：名字上像 Castagnoli，但參考實作行為吻合標準 CRC-32 (zlib) → 以參考實作為準。
    assert_eq!(h::crc32_ieee(D), 2936552237);
}

#[test]
fn empty_input() {
    assert_eq!(hex(&h::digest(&h::MD5, b"", 0)), "d41d8cd98f00b204e9800998ecf8427e");
    assert_eq!(hex(&h::digest(&h::SHA256, b"", 0)), "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855");
    assert_eq!(hex(&h::digest(&h::SHA1, b"", 0)), "da39a3ee5e6b4b0d3255bfef95601890afd80709");
    // official BLAKE3 test vector (input_len 0, first 32 bytes)
    assert_eq!(hex(&h::digest(&h::BLAKE3, b"", 0)), "af1349b9f5f9a1a6a0404dea36dcc9499bcb25c9adc112b7cc9a93cae41f3262");
    assert_eq!(h::sysv(b""), 0);
    assert_eq!(h::bsd(b""), 0);
}
