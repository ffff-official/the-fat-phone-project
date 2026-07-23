use super::*;
use crate::configure_tracing;

use crate::gsm7::Gsm7Writer;
use hex_literal::hex;
use serial_test::serial;

use std::sync::atomic::Ordering::Relaxed;

const HELLOHELLO: [u8; 24] =
    hex!("00 31 00 0B 91 64 07 28 15 53 F8 00 00 AA 0A E8 32 9B FD 46 97 D9 EC 37");
#[test]
#[serial]
fn simple_sms_hello() {
    configure_tracing();
    let result = get_sms_deliver_tpdu("46708251358", "hellohello", true).unwrap();
    assert_eq!(result[0], HELLOHELLO);
}

const SIMPLE_SMS: [u8; 29] =
    hex!("00 31 00 0B 91 64 07 28 15 53 F8 00 00 AA 10 C2 34 BD 0C 22 97 41 63 74 D9 1E 66 83 42");
#[test]
#[serial]
fn simple_sms() {
    configure_tracing();
    let smss = get_sms_deliver_tpdu("46708251358", "Bite de cheval !", true).unwrap();
    assert_eq!(smss[0], SIMPLE_SMS);
}

const LONG_LOREM_IPSUS: &str = "Lorem ipsum dolor sit amet consectetur adipiscing elit. Quisque faucibus ex sapien vitae pellentesque sem placerat. In id cursus mi pretium tellus duis convallis. Tempus leo eu aenean sed diam urna tempor. Pulvinar vivamus fringilla lacus nec metus bibendum egestas. Iaculis massa nisl malesuada lacinia integer nunc posuere. Ut hendrerit semper vel class aptent taciti sociosqu. Ad litora torquent per conubia nostra inceptos himenaeos.";

const LONG_LOREM_IPSUS_SMS_1: [u8; 155] = hex!(
    "00 71 00 0B 91 64 07 28 15 53 F8 00 00 AA A0 05 00 03 00 03 01 98 6F 79 B9 0D 4A C3 E7 F5 36 88 FC 66 BF E5 A0 79 9A 0E 0A B7 CB 74 D0 F8 ED 9E 97 C7 F4 32 BD 2E 07 85 C9 69 78 7A 3E 4E BB CF A0 32 3B 4D 77 81 A2 F5 F4 3C 5E 2F 83 CC E1 FA 38 2D AE CF 41 65 3C 68 1E 86 A7 CB 6E 90 3D 4D 0F 97 41 F0 32 9B 5D 76 D3 CB F3 78 BD 0C 9A 97 DB 20 38 3B 3C 2E CB C3 74 17 28 E9 06 A5 C9 A0 71 5D 3E AF CF 41 ED 34 08 2E 2F D3 D3 F5 36 88 5E 66 B3 EB 73 10 B9 9E 9E 83 C6"
);
const LONG_LOREM_IPSUS_SMS_2: [u8; 155] = hex!(
    "00 71 00 0B 91 64 07 28 15 53 F8 00 00 AA A0 05 00 03 00 03 02 DE 6E 7B 98 CD 4E CF 5D 20 6A B9 0D AF CF 41 EC F2 1B 54 AE 83 C2 65 77 39 EC 06 CD CB 64 10 39 1D 6E 83 EA 72 77 18 44 2F B7 E1 6F B9 0B 04 AD B3 ED 69 77 58 0E B2 A7 ED E1 76 7D 0E 32 CB D3 EE 73 9A CD 0E 83 D8 E1 71 7D 0E 72 97 C7 A0 76 99 5E 9F 83 C4 69 71 D9 4D AE B7 41 E5 73 79 4E 0F CF 5D A0 64 78 5C 67 A7 E7 A0 76 78 3E 0F 83 DC E9 39 1B D4 0E B3 CB F3 7A 98 1C 06 B1 C3 E3 B4 3B 1D 06 A5 DD"
);
const LONG_LOREM_IPSUS_SMS_3: [u8; 136] = hex!(
    "00 71 00 0B 91 64 07 28 15 53 F8 00 00 AA 8A 05 00 03 00 03 03 E8 E5 73 59 0E 72 D7 DD 63 10 FC 3D AF 97 E5 65 17 A8 4A 07 A1 CB 6E B2 BC 2C 4F D3 41 F3 72 1B 5E 96 83 EC 65 36 68 CC 0E CF E7 A0 30 9C 5E 76 D3 41 F4 F0 38 4D 4F 83 E6 EF 71 FA 3D 8F D7 5D A0 20 19 C4 4E D3 DF F2 30 88 FE 96 C7 EB 65 37 1D 04 2F CB 41 E3 B7 BB 2E 4E 87 41 EE F7 9C 2E 0F 83 D2 EE 71 19 4E 7F CF 41 E8 74 BB EC 0E 97 DF 73 17"
);
#[test]
#[serial]
fn long_gsm7_lorem_ipsum() {
    configure_tracing();
    MESSAGE_REFERENCE.store(0, Relaxed);
    let smss = get_sms_deliver_tpdu("46708251358", LONG_LOREM_IPSUS, true).unwrap();
    assert_eq!(smss[0], LONG_LOREM_IPSUS_SMS_1);
    assert_eq!(smss[1], LONG_LOREM_IPSUS_SMS_2);
    assert_eq!(smss[2], LONG_LOREM_IPSUS_SMS_3);
}

const SHORT_UTF16_MIXED: &str = "🦄🐴🐎 ! Bite de cheval ! 🐎🐴🦄";

const SHORT_UTF16_MIXED_1: [u8; 79] = hex!(
    "00 31 00 0B 91 64 07 28 15 53 F8 00 08 AA 40 D8 3E DD 84 D8 3D DC 34 D8 3D DC 0E 00 20 00 21 00 20 00 42 00 69 00 74 00 65 00 20 00 64 00 65 00 20 00 63 00 68 00 65 00 76 00 61 00 6C 00 20 00 21 00 20 D8 3D DC 0E D8 3D DC 34 D8 3E DD 84"
);
#[test]
#[serial]
fn short_utf16_mixed() {
    configure_tracing();
    let smss = get_sms_deliver_tpdu("46708251358", SHORT_UTF16_MIXED, true).unwrap();
    assert_eq!(smss[0], SHORT_UTF16_MIXED_1);
}

const ONE_LONG_SMS_HORSES: &str =
    "🐎🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐎";

const HORSES: [u8; 155] = hex!(
    "00 31 00 0B 91 64 07 28 15 53 F8 00 08 AA 8C D8 3D DC 0E D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 0E"
);
#[test]
#[serial]
fn utf16_unique_horses() {
    configure_tracing();
    let smss = get_sms_deliver_tpdu("46708251358", ONE_LONG_SMS_HORSES, true).unwrap();
    assert_eq!(smss[0], HORSES);
}

const THREE_LONG_SMS_HORSES: &str = "🐎🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐴🐎";
const THREE_LONG_SMS_HORSES_1: [u8; 153] = hex!(
    "00 71 00 0B 91 64 07 28 15 53 F8 00 08 AA 8A 05 00 03 00 03 01 D8 3D DC 0E D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34"
);
const THREE_LONG_SMS_HORSES_2: [u8; 153] = hex!(
    "00 71 00 0B 91 64 07 28 15 53 F8 00 08 AA 8A 05 00 03 00 03 02 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34"
);
const THREE_LONG_SMS_HORSES_3: [u8; 77] = hex!(
    "00 71 00 0B 91 64 07 28 15 53 F8 00 08 AA 3E 05 00 03 00 03 03 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 34 D8 3D DC 0E"
);
#[test]
#[serial]
fn three_long_sms_horses() {
    configure_tracing();
    MESSAGE_REFERENCE.store(0, Relaxed);
    let smss = get_sms_deliver_tpdu("46708251358", THREE_LONG_SMS_HORSES, true).unwrap();
    assert_eq!(smss[0], THREE_LONG_SMS_HORSES_1);
    assert_eq!(smss[1], THREE_LONG_SMS_HORSES_2);
    assert_eq!(smss[2], THREE_LONG_SMS_HORSES_3);
}

#[test]
#[serial]
#[should_panic]
fn gsm7_encode_utf16_error() {
    let mut writer = Gsm7Writer::new(Vec::new());
    writer.write_str("😂🫠😂").unwrap();
}

#[test]
#[serial]
#[should_panic]
fn gsm7_encode_ascii_error() {
    let mut writer = Gsm7Writer::new(Vec::new());
    writer.write_str(" `").unwrap();
}

#[test]
#[serial]
fn gsm7_encode_basic_valid() {
    let mut writer = Gsm7Writer::new(Vec::new());
    let s = "@£$¥èéùìòÇØøÅåΔ_ΦΓΛΩΠΨΣΘΞÆæßÉ!\"#¤%&'()*+,-./0123456789:;<=>?¡ABCDEFGHIJKLMNOPQRSTUVWXYZÄÖÑÜ§¿abcdefghijklmnopqrstuvwxyzäöñüà\n\r";
    writer.write_str(s).unwrap();
}

#[test]
#[serial]
fn gsm7_encode_extension_valid() {
    let mut writer = Gsm7Writer::new(Vec::new());
    let s = "^{}\\[~]|€a";
    writer.write_str(s).unwrap();
    dbg!(writer.get_written_bit_count());
}
