extern crate libusb;

mod print;
mod utils;

use libusb::{Context, Device, DeviceDescriptor, ConfigDescriptor, DeviceHandle, LogLevel};

use std::u16;
use std::time::Duration;

fn main() {
    test();
    return;
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
    match utils::read_device(&mut device, &device_desc, &mut handle) {
        Ok(_) => println!("Finished"),
        Err(e) => panic!("Cannot read from Device: {}", e),
    }
}

fn test() {
    let mut context = Context::new().unwrap();
    for mut d in context.devices().unwrap().iter() {
        let desc = d.device_descriptor().unwrap();
        if desc.product_id() == 50487 && desc.vendor_id() == 1133 {
            let mut handle = d.open().unwrap();
            let c = d.config_descriptor(0).unwrap();
            let iface = c.interfaces().nth(1).unwrap();
            let id = iface.descriptors().nth(0).unwrap();
            let ed = id.endpoint_descriptors().nth(0).unwrap();
            handle.detach_kernel_driver(1).unwrap();
            handle.set_active_configuration(1).unwrap();
            handle.claim_interface(1).unwrap();
            handle.set_alternate_setting(1, 0).unwrap();
            let mut buf = [0u8; 256];
            handle.read_interrupt(130, &mut buf, Duration::from_secs(1)).unwrap();
            println!("{:?}", ed);
        }
    }
}

