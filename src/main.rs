#![feature(inclusive_range_syntax)]

extern crate libusb;
extern crate pcap;

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

    let argv: Vec<String> = std::env::args().collect();
    println!("{:?}", argv);
    if argv.len() < 3 {
        println!("usage: usbtest <vendor-id> <product-id>");
        return;
    }

    let vendor_id = u16::from_str_radix(&argv[1], 16).unwrap();
    let product_id = u16::from_str_radix(&argv[2], 16).unwrap();
    println!("Vendor-Id: {}    Product-Id: {}", vendor_id, product_id);

    let mut context = utils::get_context();
    let (mut device, device_desc, mut handle) =
        match utils::open_device(&mut context, &vendor_id, &product_id) {
            Ok(t) => t,
            Err(e) => panic!("Error finding / opening device: {}", e),
    };
    use std::io;
    use std::io::prelude::*;
    let timeout = std::time::Duration::from_secs(1);
    let stdin = io::stdin();
    handle.detach_kernel_driver(0);
    handle.claim_interface(0);
    for l in stdin.lock().lines() {
        let l = l.unwrap();
        if l == "end" {
            break;
        }
        let mut buf = Vec::new();
        {
            let req = usb::Packet::from_bytes(cap.next().unwrap().data).unwrap();
            buf.resize(req.get_w_length() as usize, 0u8);
            println!("{:?}", req);
            println!("{}", buf.len());
            println!("{}", (&buf[..]).len());
            println!("{}", handle.read_control(req.get_bm_request_type(), req.get_b_request(), req.get_value(), req.get_language_id(), &mut buf, timeout).unwrap());
            println!("{:?}", buf);
        }
        {
            let res = usb::Packet::from_bytes(cap.next().unwrap().data).unwrap();
            println!("{:?}", res);
            println!("Result correct: {}", res.get_data() == &buf[..]);
        }
    }
    handle.release_interface(0);
    handle.attach_kernel_driver(0);
    return;
    match utils::read_device(&mut device, &device_desc, &mut handle) {
        Ok(_) => println!("Finished"),
        Err(e) => panic!("Cannot read from Device: {}", e),
    }
}

