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
use keys::*;

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
    for d in devices.iter() {
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

pub fn print_all_data(p: &Path) {
    let mut c = pcap::Capture::from_file(p).unwrap();
    while let Ok(pa) = c.next() {
        let packet = usb::Packet::from_bytes(pa.data).unwrap();
        if packet.get_direction() == usb::Direction::Out
                && packet.get_data_length() != 0
                && packet.get_endpoint() == 0
                && packet.get_data()[0] == 0x12 {
            println!("{:?}", packet.get_data().iter().map(|b| format!("{:02x}", b)).collect::<Vec<_>>());
        }
    }
}

#[allow(unused)]
pub fn compare(p1: &Path, p2: &Path) {
    let mut c1 = pcap::Capture::from_file(&p1).unwrap();
    let mut c2 = pcap::Capture::from_file(&p2).unwrap();
    for i in 0.. {
        let (packet1, packet2) = match (c1.next(), c2.next()) {
            (Ok(p1), Ok(p2)) => (p1, p2),
            (Err(e), Ok(p2)) => return println!("capture1 is empty, capture2 still has sth ({:?})", e),
            (Ok(p1), Err(e)) => return println!("capture1 still has sth, capture2 is empty ({:?})", e),
            (Err(e1), Err(e2)) => return println!("success ({:?}, {:?})", e1, e2),

        };
        let p1 = usb::Packet::from_bytes(packet1.data).unwrap();
        let p2 = usb::Packet::from_bytes(packet2.data).unwrap();
        if !p1.same(&p2) {
            println!("Packet {} incorrect", i+1);
            println!("{}", format!("{:?}", p1).replace(", ", ",\n").replace("{ ", "{\n"));
            println!("{}", format!("{:?}", p2).replace(", ", ",\n").replace("{ ", "{\n"));
        }
    }
}

#[allow(unused)]
pub fn print_memory_layout() {
    let gaming_key_offsets = [
        0xFFFFFFFF, 0x382E2402, 0x92887E02, 0x584E4402,
        0x766C6202, 0x948A8002, 0x362C2202, 0x342A2002,
        0x32281E02, 0xAAA09601
    ];
    let standard_key_offsets = [
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xAEA49A02, 0x32281E01, 0xE6DCD202, 0xC8BEB402,
        0x8C827802, 0xAAA09602, 0x6E645A01, 0x160C0201,
        0x90867C01, 0x180E0401, 0x72685E01, 0x746A6001,
        0x362C2201, 0x342A2001, 0x92887E01, 0xECE2D801,
        0x72685E02, 0x6E645A02, 0xACA29802, 0x8C827801,
        0x52483E01, 0x140A0001, 0x8E847A02, 0xCAC0B602,
        0x70665C01, 0x90867C02, 0x544A4002, 0x52483E02,
        0x70665C02, 0x50463C02, 0xE6DCD201, 0xE8DED401,
        0x8E847A01, 0xEAE0D601, 0xCCC2B801, 0xCEC4BA01,
        0x6E645A00, 0x1A100602, 0xC8BEB400, 0x746A6002,
        0x50463C01, 0xEEE4DA01, 0x948A8001, 0x766C6201,
        0x8C827800, 0x8E847A00, 0xAAA09600, 0x1A100601,
        0x1C120801, 0x564C4202, 0x544A4001, 0x382E2401,
        0x3A302601, 0xB0A69C02, 0x180E0402, 0x160C0202,
        0x140A0002, 0xC8BEB401, 0xCAC0B601, 0xACA29801,
        0xAEA49A01, 0xB0A69C01, 0xB2A89E01, 0xD0C6BC01,
        0xE6DCD200, 0xE8DED400, 0xCAC0B600, 0xCCC2B800,
        0x180E0400, 0xACA29800, 0xAEA49A00, 0xEAE0D600,
        0x70665C00, 0x72685E00, 0x90867C00, 0x544A4000,
        0x342A2000, 0x52483E00, 0x160C0200, 0xCEC4BA00,
        0xECE2D800, 0xEEE4DA00, 0xD0C6BC00, 0x948A8000,
        0x584E4400, 0x382E2400, 0x1A100600, 0x1C120800,
        0x564C4200, 0x746A6000, 0x766C6200, 0x92887E00,
        0xB0A69C00, 0xB2A89E00, 0x362C2200, 0x3A302600,
        0xCCC2B802, 0x50463C00, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0x1C120802,
        0x564C4201, 0x3A302602, 0xB2A89E02, 0xD0C6BC02,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF, 0xFFFFFFFF,
        0xECE2D802, 0xCEC4BA02, 0xE8DED402, 0xEAE0D602,
        0x32281E00, 0x140A0000, 0x564C4201, 0x584E4401,
        0xFF0000FF, 0x23FF0011, 0x5FFF00, 0xFF00FBFF,
        0xFF5E00FF, 0xFF2000, 0xFF0E, 0xFF0000FF,
        0x11FF0000, 0x23FF00, 0xFF005FFF, 0xFFFF00FB,
        0xFF5E00, 0xE00FF20, 0xFF0000FF, 0x11FF0000,
        0x23FF00, 0xFF005FFF, 0xFFFF00FB, 0xFFFF00,
        0x2000FF5E, 0xFF0E00FF, 0x484B0000, 0x78424D
    ];

    let getoffsets = |key_code, arr: &[u32]| {
        let offset = arr[key_code as usize];
        let base_offset = 0xf0usize * (offset as u8) as usize + 0x2b;
        (base_offset + (offset >> 8) as u8 as usize, base_offset + (offset >> 16) as u8 as usize,
            base_offset + (offset >> 24) as u8 as usize)
    };

    //let mut memory: Vec<String> = Vec::new();
    //memory.resize(1000, "".to_string());
    let mut memory: Vec<u8> = Vec::new();
    memory.resize(1000, 0u8);
    for key in StandardKey::values() {
        if key == StandardKey::None {
            continue;
        }
        let (r,g,b) = getoffsets(key as u8, &standard_key_offsets);
        //memory[r] = format!("r {:?}", key);
        //memory[g] = format!("g {:?}", key);
        //memory[b] = format!("b {:?}", key);
        memory[r] = key as u8;
        memory[g] = key as u8;
        memory[b] = key as u8;
    }
    for key in GamingKey::values() {
        if key == GamingKey::None {
            continue;
        }
        let (r,g,b) = getoffsets(key as u8, &gaming_key_offsets);
        //memory[r] = format!("r {:?}", key);
        //memory[g] = format!("g {:?}", key);
        //memory[b] = format!("b {:?}", key);
        memory[r] = key as u8;
        memory[g] = key as u8;
        memory[b] = key as u8;
    }
    for c in (&memory[42..282]).chunks(10) {
        println!("{:02x}, {:02x}, {:02x}, {:02x}, {:02x}", c[1], c[3], c[5], c[7], c[9]);
    }
    println!("");
    for c in (&memory[282..522]).chunks(10) {
        println!("{:02x}, {:02x}, {:02x}, {:02x}, {:02x}", c[1], c[3], c[5], c[7], c[9]);
    }
    println!("");
    for c in (&memory[522..762]).chunks(10) {
        println!("{:02x}, {:02x}, {:02x}, {:02x}, {:02x}", c[1], c[3], c[5], c[7], c[9]);
    }
    let test: Vec<_> = (0..0xff).filter(|n| !memory.contains(n)).collect();
    println!("{:?}", &test[..]);
}
