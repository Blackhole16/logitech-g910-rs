use libusb;
use libusb::{
    Context,
    Device,
    DeviceDescriptor,
    ConfigDescriptor,
    Version,
    Result,
    Interfaces,
    Interface,
    InterfaceDescriptor,
    EndpointDescriptor,
};
use pcap::{Capture, Offline};
use usb::Packet;

trait PrintPrefix {
    fn to_str(&self) -> &str;
}

impl <'a> PrintPrefix for Option<&'a str> {
    fn to_str(&self) -> &str {
        match *self {
            Some(p) => p,
            None => "",
        }
    }
}

#[allow(unused)]
pub fn print_libusb() {
    let version = libusb::version();
    println!("libusb v{}.{}.{}.{}{}", version.major(), version.minor(), version.micro(), version.nano(), version.rc().unwrap_or(""));
}

#[allow(unused)]
pub fn print_context(context: &Context) {
    println!("has capability? {}", context.has_capability());
    println!("has hotplug? {}", context.has_hotplug());
    println!("has HID access? {}", context.has_hid_access());
    println!("supports detach kernel driver? {}", context.supports_detach_kernel_driver());
}

#[allow(unused)]
pub fn print_everything(device: &mut Device) {
    println!("Device:");
    print_device(device, Some("    "));
    match device.device_descriptor() {
        Ok(desc) => {
            println!("Descriptor:");
            print_descriptor(&desc, Some("    "));
        },
        Err(e) => println!("Error accessing descriptor: {:?}", e),
    }
    println!("Config:");
    print_configs(device, Some("    ")).unwrap();
    println!("");
}

#[allow(unused)]
pub fn print_device(device: &mut Device, prefix: Option<&str>) {
    println!("{}Bus: {}", prefix.to_str(), device.bus_number());
    println!("{}Address: {}", prefix.to_str(), device.address());
    println!("{}Speed: {:?}", prefix.to_str(), device.speed());
}

#[allow(unused)]
pub fn print_descriptor(desc: &DeviceDescriptor, prefix: Option<&str>) {
    println!("{}UsbVersion: {}", prefix.to_str(), version_to_string(&desc.usb_version()));
    println!("{}DeviceVersion: {}", prefix.to_str(), version_to_string(&desc.device_version()));
    println!("{}ClassCode: {}", prefix.to_str(), desc.class_code());
    println!("{}SubClassCode: {}", prefix.to_str(), desc.sub_class_code());
    println!("{}ProtocolCode: {}", prefix.to_str(), desc.protocol_code());
    println!("{}VendorId: {}", prefix.to_str(), desc.vendor_id());
    println!("{}ProductId: {}", prefix.to_str(), desc.product_id());
    println!("{}    {:04x}:{:04x}", prefix.to_str(), desc.vendor_id(), desc.product_id());
    println!("{}MaxPacketSize: {}", prefix.to_str(), desc.max_packet_size());
    println!("{}NumConfigurations: {}", prefix.to_str(), desc.num_configurations());
}

#[allow(unused)]
pub fn print_configs(device: &mut Device, prefix: Option<&str>) -> Result<()> {
    let desc = try!(device.device_descriptor());
    let num_config = desc.num_configurations();
    for i in 0..num_config {
        let config = try!(device.config_descriptor(i));
        print_config(&config, prefix);
    }
    return Ok(());
}

#[allow(unused)]
pub fn print_config(config: &ConfigDescriptor, prefix: Option<&str>) {
    println!("{}Num: {}", prefix.to_str(), config.number());
    println!("{}MaxPower: {}", prefix.to_str(), config.max_power());
    println!("{}SelfPowered: {}", prefix.to_str(), config.self_powered());
    println!("{}RemoteWakeup: {}", prefix.to_str(), config.remote_wakeup());
    println!("{}NumInterfaces: {}", prefix.to_str(), config.num_interfaces());
    println!("{}Interfaces:", prefix.to_str());
    print_interfaces(&mut config.interfaces(), Some(&(prefix.to_str().to_string() + "    ")));
}

#[allow(unused)]
pub fn print_interfaces(interfaces: &mut Interfaces, prefix: Option<&str>) {
    for interface in interfaces {
        print_interface(&interface, prefix);
    }
}

#[allow(unused)]
pub fn print_interface(interface: &Interface, prefix: Option<&str>) {
    println!("{}Number: {}", prefix.to_str(), interface.number());
    for if_desc in interface.descriptors() {
        print_interface_descriptor(&if_desc, prefix);
    }
}

#[allow(unused)]
pub fn print_interface_descriptor(if_desc: &InterfaceDescriptor, prefix: Option<&str>) {
    println!("{}Number: {}", prefix.to_str(), if_desc.interface_number());
    println!("{}SettingNumber: {}", prefix.to_str(), if_desc.setting_number());
    println!("{}ClassCode: {}", prefix.to_str(), if_desc.class_code());
    println!("{}SubClassCode: {}", prefix.to_str(), if_desc.sub_class_code());
    println!("{}ProtocolCode: {}", prefix.to_str(), if_desc.protocol_code());
    println!("{}NumEndpoints: {}", prefix.to_str(), if_desc.num_endpoints());
    for endpoint in if_desc.endpoint_descriptors() {
        println!("{}Endpoint:", prefix.to_str());
        print_endpoint(&endpoint, Some(&(prefix.to_str().to_string() + "    ")));
    }
}

#[allow(unused)]
pub fn print_endpoint(endpoint: &EndpointDescriptor, prefix: Option<&str>) {
    println!("{}Address: {}", prefix.to_str(), endpoint.address());
    println!("{}Number: {}", prefix.to_str(), endpoint.number());
    println!("{}Direction: {:?}", prefix.to_str(), endpoint.direction());
    println!("{}TransferType: {:?}", prefix.to_str(), endpoint.transfer_type());
    println!("{}SyncType: {:?}", prefix.to_str(), endpoint.sync_type());
    println!("{}UsageType: {:?}", prefix.to_str(), endpoint.usage_type());
    println!("{}MaxPacketSize: {}", prefix.to_str(), endpoint.max_packet_size());
    println!("{}Interval: {}", prefix.to_str(), endpoint.interval());
}

#[allow(unused)]
fn version_to_string(v: &Version) -> String {
    let &Version(j, m, n) = v;
    return format!("v{}.{}.{}", j, m, n);
}

#[allow(unused)]
pub fn print_cap(cap: &mut Capture<Offline>) {
    while let Ok(packet) = cap.next() {
        println!("Packet: {:?}", packet);
        if packet.header.len < 64 {
            unreachable!();
        }
        let p = Packet::from_bytes(&packet.data).unwrap();
        println!("{:?}", p);

    }
}

