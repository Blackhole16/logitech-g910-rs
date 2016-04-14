extern crate libusb;

mod print;
mod utils;

use libusb::{Context, Device, DeviceDescriptor, ConfigDescriptor, DeviceHandle, LogLevel};

fn main() {
    let argv: Vec<String> = std::env::args().collect();
    println!("{:?}", argv);
    if argv.len() < 3 {
        println!("usage: usbtest <vendor-id> <product-id>");
        return;
    }

    let vendor_id: u16 = argv[1].parse().unwrap();
    let product_id: u16 = argv[2].parse().unwrap();
    println!("Vendor-Id: {}    Product-Id: {}", vendor_id, product_id);

    let mut context = utils::get_context();
    let (mut device, device_desc, mut handle) =
        match utils::open_device(&mut context, &vendor_id, &product_id) {
            Ok(t) => t,
            Err(e) => panic!("Error finding / opening device: {}", e),
    };
    match utils::read_device(&mut device, &device_desc, &mut handle) {
        Ok(_) => println!("Finished"),
        Err(e) => panic!("Cannot read from Device: {}", e),
    }
}

