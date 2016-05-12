use libusb::{
    LogLevel,
    Context,
    Device,
    DeviceDescriptor,
    DeviceHandle,
    Direction,
    TransferType,
    Result,
    Error,
};
use std::time::Duration;
use std::fmt::Display;
use std::path::Path;
use pcap;
use usb;

#[allow(dead_code)]
#[derive(Debug)]
pub struct Endpoint {
    config: u8,
    iface: u8,
    setting: u8,
    address: u8,
    transfer_type: TransferType,
}

trait PrintResult {
  fn to_string(&self) -> String;
}

impl<T> PrintResult for Result<T> where T: Display {
  fn to_string(&self) -> String {
    match *self {
      Ok(ref x) => x.to_string(),
      Err(ref e) => format!("{:?}", e)
    }
  }
}

pub fn get_context() -> Context {
    let mut context = match Context::new() {
        Ok(c) => c,
        Err(e) => panic!("Context::new(): {}", e)
    };
    context.set_log_level(LogLevel::Debug);
    context.set_log_level(LogLevel::Info);
    context.set_log_level(LogLevel::Warning);
    context.set_log_level(LogLevel::Error);
    context.set_log_level(LogLevel::None);
    return context;
}

pub fn open_device(context: &Context, vendor_id: u16, product_id: u16) -> Result<(Device, DeviceDescriptor, DeviceHandle)> {
    let devices = match context.devices() {
        Ok(devices) => devices,
        Err(e) => return Err(e),
    };
    for mut d in devices.iter() {
        let dd = match d.device_descriptor() {
            Ok(dd) => dd,
            Err(_) => continue
        };
        if dd.vendor_id() == vendor_id && dd.product_id() == product_id  {
            return match d.open() {
                Ok(handle) => Ok((d, dd, handle)),
                Err(e) => Err(e),
            }
        }
    }
    return Err(Error::NoDevice);
}

#[allow(unused)]
pub fn read_device(device: &mut Device, device_desc: &DeviceDescriptor, handle: &mut DeviceHandle) -> Result<()> {
    try!(handle.reset());

    let timeout = Duration::from_secs(1);
    let languages = try!(handle.read_languages(timeout));
    println!("Active Configuration: {}", try!(handle.active_configuration()));
    println!("Languages: {:?}", languages);

    if languages.len() > 0 {
        let lang = languages[0];
        println!("Manufacturer: {}", 
                 handle.read_manufacturer_string(lang, device_desc, timeout).to_string());
        println!("Product: {}", 
                 handle.read_product_string(lang, device_desc, timeout).to_string());
        println!("Serial Number: {}", 
                 handle.read_serial_number_string(lang, device_desc, timeout).to_string());
    }


    for endpoint in get_readable_endpoints(device, device_desc) {
        println!("Got readable endpoint: {:?}", endpoint);
        if endpoint.iface == 1 {
            if let Ok(b) = handle.kernel_driver_active(endpoint.iface){
                println!("    Kernel driver active: {}", b);
            }
            read_endpoint(handle, &endpoint).unwrap();
        }
    }
    println!("");
    for endpoint in get_writable_endpoints(device, device_desc) {
        println!("Got writable endpoint: {:?}", endpoint);
    }

    return Ok(());
}

#[allow(unused)]
pub fn get_readable_endpoints(device: &mut Device, device_desc: &DeviceDescriptor) -> Vec<Endpoint> {
    get_endpoints(device, device_desc, Direction::In)
}

#[allow(unused)]
pub fn get_writable_endpoints(device: &mut Device, device_desc: &DeviceDescriptor) -> Vec<Endpoint> {
    get_endpoints(device, device_desc, Direction::Out)
}

#[allow(unused)]
pub fn get_endpoints(device: &mut Device, device_desc: &DeviceDescriptor, dir: Direction) -> Vec<Endpoint> {
    let mut endpoints = Vec::new();
    for i in 0..device_desc.num_configurations() {
        let config_desc = match device.config_descriptor(i) {
            Ok(c) => c,
            Err(_) => continue
        };

        for interface in config_desc.interfaces() {
            for interface_desc in interface.descriptors() {
                for endpoint_desc in interface_desc.endpoint_descriptors() {
                    if endpoint_desc.direction() == dir {
                        endpoints.push(Endpoint {
                            config: config_desc.number(),
                            iface: interface_desc.interface_number(),
                            setting: interface_desc.setting_number(),
                            address: endpoint_desc.address(),
                            transfer_type: endpoint_desc.transfer_type(),
                        });
                    }
                }
            }
        }
    }
    return endpoints;
}

pub fn detach(handle: &mut DeviceHandle, iface: u8) -> Result<bool> {
    match handle.kernel_driver_active(iface) {
        Ok(true) => {
            try!(handle.detach_kernel_driver(iface));
            Ok(true)
        },
        _ => Ok(false)
    }

}

#[allow(unused)]
fn read_endpoint(handle: &mut DeviceHandle, endpoint: &Endpoint) -> Result<()>{
    let has_kernel_driver = detach(handle, endpoint.iface).unwrap();
    println!("    Kernel driver active for iface {}: {}", endpoint.iface, has_kernel_driver);
    // we also need to be able to write to / read from interface 0, otherwise
    // set_active_configuration reports a busy device
    let has_kernel_driver0 = detach(handle, 0).unwrap();
    println!("    Kernel driver active for iface 0: {}", has_kernel_driver);
    
    let timeout = Duration::from_secs(1);
    try!(handle.reset());
    for lang in handle.read_languages(timeout).unwrap() {
        println!("Got lang: {:?}", lang);
        for i in 0...255u8 {
            if let Ok(s) = handle.read_string_descriptor(lang, 0u8, timeout) {
                println!("got desc {}: {}", i, s);
            }
        }
    }
    println!("0");
    //try!(handle.unconfigure());
    println!("1");
    try!(handle.set_active_configuration(1));
    println!("2");
    //try!(handle.claim_interface(endpoint.iface));
    try!(handle.claim_interface(0));
    println!("3");
    //try!(handle.set_alternate_setting(endpoint.iface, endpoint.setting));
    try!(handle.set_alternate_setting(0, 0));
    println!("4");

    let mut buf = [0u8; 8];
    println!("start reading {} bytes", buf.len());
    loop {
        match handle.read_interrupt(129, &mut buf, timeout) {
            Ok(len) => {
                print!("read {} bytes: ", len);
                println!("{:?}", buf);
            },
            Err(e) => {
                print!("ERROR reading: {:?}", e);
                break;
            }
        }
    }

    match handle.release_interface(endpoint.iface) {
        Err(e) => println!("Could not release iface {}: {}", endpoint.iface, e),
        _ => {}
    }
    match handle.release_interface(0) {
        Err(e) => println!("Could not release iface 0: {}", e),
        _ => {}
    }

    // reattach kernel driver(s)
    if has_kernel_driver {
        match handle.attach_kernel_driver(endpoint.iface) {
            Err(e) => println!("Error attaching kernel driver for iface {}: {}", endpoint.iface, e),
            _ => {}
        }
    }
    if has_kernel_driver0 {
        match handle.attach_kernel_driver(0) {
            Err(e) => println!("Error attaching kernel driver for iface 0: {}", e),
            _ => {}
        }
    }
    return Ok(());
}

pub fn compare(p1: &Path, p2: &Path) {
    let mut c1 = pcap::Capture::from_file(&p1).unwrap();
    let mut c2 = pcap::Capture::from_file(&p2).unwrap();
    for i in 0..110 {
        let p1 = usb::Packet::from_bytes(c1.next().unwrap().data).unwrap();
        let p2 = usb::Packet::from_bytes(c2.next().unwrap().data).unwrap();
        if !p1.same(&p2) {
            println!("Packet {} incorrect", i+1);
            println!("{:?}", p1);
            println!("{:?}", p2);
        }
    }
}
