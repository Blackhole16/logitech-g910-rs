use libusb::{Device, DeviceDescriptor, ConfigDescriptor, DeviceHandle, Version};

pub fn print_all(device: &mut Device) {
    println!("Device:");
    print_device_prefix(device, "    ");
    match device.device_descriptor() {
        Ok(desc) => {
            println!("Descriptor:");
            print_descriptor_prefix(&desc, "    ");
        },
        Err(e) => println!("Error accessing descriptor: {:?}", e),
    }
    println!("");
}

pub fn print_device(device: &mut Device) {
    print_device_prefix(device, "");
}

pub fn print_device_prefix(device: &mut Device, prefix: &str) {
    println!("{}Bus: {}", prefix, device.bus_number());
    println!("{}Address: {}", prefix, device.address());
    println!("{}Speed: {:?}", prefix, device.speed());
}

pub fn print_descriptor(desc: &DeviceDescriptor) {
    print_descriptor_prefix(desc, "");
}

pub fn print_descriptor_prefix(desc: &DeviceDescriptor, prefix: &str) {
    println!("{}USB-Version: {}", prefix, version_to_string(&desc.usb_version()));
    println!("{}Device-Version: {}", prefix, version_to_string(&desc.device_version()));
    println!("{}")
}


fn version_to_string(v: &Version) -> String {
    let &Version(j, m, n) = v;
    return format!("{}.{}.{}", j, m, n);
}
