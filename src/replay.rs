use pcap::{Capture, Offline};
use std::path::Path;
use usb::Packet;

pub fn get_capture(path: &Path) -> Capture<Offline> {
    return Capture::from_file(path).unwrap();
}

pub fn print(cap: &mut Capture<Offline>) {
    while let Ok(packet) = cap.next() {
        println!("Packet: {:?}", packet);
        if packet.header.len < 64 {
            unreachable!();
        }
        let p = Packet::from_bytes(&packet.data).unwrap();
        println!("{:?}", p);

    }
}
