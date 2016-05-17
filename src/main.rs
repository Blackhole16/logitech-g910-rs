#![feature(inclusive_range_syntax)]

extern crate libusb;
extern crate pcap;

mod consts;
mod print;
mod replay;
mod usb;
mod utils;

use std::path::Path;
use replay::Control;

fn main() {
    let context = utils::get_context();
    let p = Path::new("pcap/g910-handshake.pcap");
    //let p = Path::new("pcap/g602-handshake.pcap");
    let (_, _, mut handle) = utils::open_device(&context, consts::VENDOR_ID, consts::PRODUCT_ID).unwrap();

    // for some reason we cannot claim interface 2 as it doesn't exist
    // but we will be able to read from it, if we claim interface 1
    println!("Claiming interfaces 0 and 1");
    // detch kernel driver
    let has_kernel_driver0 = utils::detach(&mut handle, 0).unwrap();
    let has_kernel_driver1 = utils::detach(&mut handle, 1).unwrap();
    handle.claim_interface(0).unwrap();
    handle.claim_interface(1).unwrap();

    {
        println!("resetting handle");
        handle.reset().unwrap();
        let mut ctrl = Control::new(&p, &context, &handle);
        // first 6 packets are from wireshark
        ctrl.skip(6);
        ctrl.replay_handshake().unwrap();
    }

    handle.release_interface(1).unwrap();
    handle.release_interface(0).unwrap();
    if has_kernel_driver1 {
        handle.attach_kernel_driver(1).unwrap();
    }
    if has_kernel_driver0 {
        handle.attach_kernel_driver(0).unwrap();
    }
}

