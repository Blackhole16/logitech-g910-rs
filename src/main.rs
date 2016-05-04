#![feature(inclusive_range_syntax)]

extern crate libusb;
extern crate pcap;

mod consts;
mod print;
mod utils;
mod replay;
mod usb;

use std::path::Path;
use replay::Control;

fn main() {
    let mut context = utils::get_context();
    //let p = Path::new("pcap/g910-handshake.pcap");
    let p = Path::new("pcap/g602-handshake.pcap");
    let mut ctrl = Control::new(&p, &mut context);
    // first 6 packets are from wireshark
    ctrl.skip(6);

    //let argv: Vec<String> = std::env::args().collect();
    //println!("{:?}", argv);
    //if argv.len() < 3 {
        //println!("usage: usbtest <vendor-id> <product-id>");
        //return;
    //}

    //let vendor_id = u16::from_str_radix(&argv[1], 16).unwrap();
    //let product_id = u16::from_str_radix(&argv[2], 16).unwrap();
    //println!("Vendor-Id: {}    Product-Id: {}", vendor_id, product_id);

    //ctrl.replay_all().unwrap();
    ctrl.test().unwrap();
    //match utils::read_device(&mut device, &device_desc, &mut handle) {
        //Ok(_) => println!("Finished"),
        //Err(e) => panic!("Cannot read from Device: {}", e),
    //}
}

