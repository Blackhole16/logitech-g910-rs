use pcap::{Capture, Offline};
use std::path::Path;
use libusb::{DeviceHandle, Result, Device};
use usb::{Packet, TransferType};
use std::time::Duration;

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

pub fn handle(device: &mut Device, handle: &mut DeviceHandle, packet: &Packet) -> Result<()> {
    let timeout = Duration::from_secs(1);
    match packet.get_transfer_type() {
        TransferType::Interrupt => {
            //let buf = packet.get_buf();
            let buf = &[1u8; 4][..];
            match handle.write_interrupt(packet.get_endpoint(), buf, timeout) {
                Ok(len) => {
                    println!("Wrote {} bytes", len);
                    return Ok(());
                },
                Err(e) => return Err(e)
            }
        },
        _ => unimplemented!()
    }
}

