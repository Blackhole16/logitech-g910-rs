#![feature(inclusive_range_syntax)]

extern crate libusb;
extern crate pcap;

mod consts;
mod print;
mod utils;
mod replay;
mod usb;

use std::u16;
use std::path::Path;

fn main() {
    let mut cap = replay::get_capture(&Path::new("pcap/g602-handshake.pcap"));
    //replay::print(&mut cap);
    cap.next();cap.next();cap.next();cap.next();cap.next();cap.next();

    //let argv: Vec<String> = std::env::args().collect();
    //println!("{:?}", argv);
    //if argv.len() < 3 {
        //println!("usage: usbtest <vendor-id> <product-id>");
        //return;
    //}

    //let vendor_id = u16::from_str_radix(&argv[1], 16).unwrap();
    //let product_id = u16::from_str_radix(&argv[2], 16).unwrap();
    //println!("Vendor-Id: {}    Product-Id: {}", vendor_id, product_id);

    let mut context = utils::get_context();
    let (mut device, device_desc, mut handle) =
        match utils::open_device(&mut context, &consts::vendor_id, &consts::product_id) {
            Ok(t) => t,
            Err(e) => panic!("Error finding / opening device: {}", e),
    };
    replay::replay(&mut device, &mut handle, &mut cap);
    //match utils::read_device(&mut device, &device_desc, &mut handle) {
        //Ok(_) => println!("Finished"),
        //Err(e) => panic!("Cannot read from Device: {}", e),
    //}
}

