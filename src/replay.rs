use pcap::{Capture, Offline};
use std::path::Path;
use usb::{PacketBytes, Packet};

pub fn getCapture(path: &Path) -> Capture<Offline> {
    return Capture::from_file(path).unwrap();
}

pub fn print(cap: &mut Capture<Offline>) {
    while let Ok(packet) = cap.next() {
        println!("Packet: {:?}", packet);
        if packet.header.len < 64 {
            unreachable!();
        }
        let pb = PacketBytes::from_bytes(&packet.data).unwrap();
        println!("{:?}", pb);
        let p = Packet::from_bytes(&packet.data).unwrap();
        println!("{:?}", p);

    }
}
