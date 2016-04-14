extern crate libusb;

mod print;

use libusb::{Context, Device, DeviceDescriptor, ConfigDescriptor, DeviceHandle, LogLevel};

fn main() {
    print::print_libusb();

    let mut context = match Context::new() {
        Ok(c) => c,
        Err(e) => panic!("Context::new(): {}", e)
    };

    context.set_log_level(LogLevel::Debug);
    context.set_log_level(LogLevel::Info);
    context.set_log_level(LogLevel::Warning);
    context.set_log_level(LogLevel::Error);
    context.set_log_level(LogLevel::None);

    print::print_context(&mut context);

    for mut device in context.devices().unwrap().iter() {
        print::print_everything(&mut device);

        //if device_desc.vendor_id() == 1133 && device_desc.product_id() == 49963 {
            //println!("{:?}", device.speed())
        //}
    }
}
