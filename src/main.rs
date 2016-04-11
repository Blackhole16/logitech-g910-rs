extern crate libusb;

mod print;

use libusb::{Context, Device, DeviceDescriptor, ConfigDescriptor, DeviceHandle, LogLevel};

fn main() {
    let version = libusb::version();

    println!("libusb v{}.{}.{}.{}{}", version.major(), version.minor(), version.micro(), version.nano(), version.rc().unwrap_or(""));

    let mut context = match Context::new() {
        Ok(c) => c,
        Err(e) => panic!("Context::new(): {}", e)
    };

    context.set_log_level(LogLevel::Debug);
    context.set_log_level(LogLevel::Info);
    context.set_log_level(LogLevel::Warning);
    context.set_log_level(LogLevel::Error);
    context.set_log_level(LogLevel::None);

    println!("has capability? {}", context.has_capability());
    println!("has hotplug? {}", context.has_hotplug());
    println!("has HID access? {}", context.has_hid_access());
    println!("supports detach kernel driver? {}", context.supports_detach_kernel_driver());

    for mut device in context.devices().unwrap().iter() {
        print::print_all(&mut device);

        //if device_desc.vendor_id() == 1133 && device_desc.product_id() == 49963 {
            //println!("{:?}", device.speed())
        //}
    }
}
