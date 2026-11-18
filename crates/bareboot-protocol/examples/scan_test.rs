use bareboot_protocol::{scan, FrameParse, crc16_ccitt};
fn main() {
    let data = std::fs::read("/run/media/sun/halo/bareBoot/test-firmware/bp-blink-app/target/thumbv7m-none-eabi/release/bp-blink-app.bin").unwrap();
    let mut payload = Vec::new();
    payload.extend_from_slice(&0x08002000u32.to_le_bytes());
    payload.extend_from_slice(&data[..64]);
    let body = [vec![0x12u8, 0x11], payload.clone()].concat();
    let crc = crc16_ccitt(&body);
    let frame = [vec![0x01,0x88], body, crc.to_le_bytes().to_vec(), vec![0x99,0x03]].concat();
    println!("frame len {} crc {:04x} frame {}", frame.len(), crc, frame.iter().map(|b| format!("{:02x}",b)).collect::<String>());
    match scan(&frame) {
        FrameParse::Complete { frame_len, command, word_count, payload_offset } =>
            println!("COMPLETE len={} cmd={:02x} wc={} off={}", frame_len, command, word_count, payload_offset),
        FrameParse::Invalid { drop } => println!("INVALID drop={}", drop),
        FrameParse::NeedMore => println!("NEEDMORE"),
    }
}
