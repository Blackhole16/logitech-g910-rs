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
use std::thread::sleep;

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

pub fn open_device<'a>(context: &'a mut Context, vendor_id: &u16, product_id: &u16) -> Result<(Device<'a>, DeviceDescriptor, DeviceHandle<'a>)> {
    let devices = match context.devices() {
        Ok(devices) => devices,
        Err(e) => return Err(e),
    };
    for mut d in devices.iter() {
        let dd = match d.device_descriptor() {
            Ok(dd) => dd,
            Err(_) => continue
        };
        if dd.vendor_id() == *vendor_id && dd.product_id() == *product_id  {
            return match d.open() {
                Ok(handle) => Ok((d, dd, handle)),
                Err(e) => Err(e),
            }
        }
    }
    return Err(Error::NoDevice);
}

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
            handle.kernel_driver_active(endpoint.iface).map(|b| println!("    Kernel driver active: {}", b));
            read_endpoint(handle, &endpoint).unwrap();
        }
    }
    println!("");
    for endpoint in get_writable_endpoints(device, device_desc) {
        println!("Got writable endpoint: {:?}", endpoint);
    }

    return Ok(());
}

pub fn get_readable_endpoints(device: &mut Device, device_desc: &DeviceDescriptor) -> Vec<Endpoint> {
    get_endpoints(device, device_desc, Direction::In)
}

pub fn get_writable_endpoints(device: &mut Device, device_desc: &DeviceDescriptor) -> Vec<Endpoint> {
    get_endpoints(device, device_desc, Direction::Out)
}

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

fn read_endpoint(handle: &mut DeviceHandle, endpoint: &Endpoint) -> Result<()>{
    let has_kernel_driver = match handle.kernel_driver_active(endpoint.iface) {
        Ok(true) => {
            handle.detach_kernel_driver(endpoint.iface);
            true
        },
        _ => false
    };
    println!("    Kernel driver active for iface {}: {}", endpoint.iface, has_kernel_driver);
    // we also need to be able to write to / read from interface 0, otherwise
    // set_active_configuration reports a busy device
    let has_kernel_driver0 = match handle.kernel_driver_active(0) {
        Ok(true) => {
            handle.detach_kernel_driver(0);
            true
        },
        _ => false
    };
    println!("    Kernel driver active for iface 0: {}", has_kernel_driver);
    
    let timeout = Duration::from_secs(1);
    try!(handle.reset());
    for lang in handle.read_languages(timeout).unwrap() {
        println!("Got lang: {:?}", lang);
        for i in 0..256u8 {
            handle.read_string_descriptor(lang, 0u8, timeout).map(|s| println!("got desc {}: {}", i, s));
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
    while match handle.read_interrupt(129, &mut buf, timeout) {
        Ok(len) => { print!("read {} bytes: ", len); true },
        Err(e) => { print!("ERROR reading: {:?}", e); false }
    } {
        println!("{}, {}, {}, {}, {}, {}, {}, {}", &buf[0], &buf[1], &buf[2], &buf[3], &buf[4], &buf[5], &buf[6], &buf[7]);
    }

    handle.release_interface(endpoint.iface);
    handle.release_interface(0);

    // reattach kernel driver(s)
    if has_kernel_driver {
        handle.attach_kernel_driver(endpoint.iface);
    }
    if has_kernel_driver0 {
        handle.attach_kernel_driver(0);
    }
    return Ok(());
}
