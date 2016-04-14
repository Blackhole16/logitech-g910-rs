use libusb::{
    LogLevel,
    Context,
    Device,
    DeviceDescriptor,
    DeviceHandle,
    Result,
    Error,
};
use std::time::Duration;

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
        println!("Manufacturer: {}", try!(handle.read_manufacturer_string(lang, device_desc, timeout)));
        println!("Product: {}", try!(handle.read_product_string(lang, device_desc, timeout)));
        println!("Serial Number: {}", try!(handle.read_serial_number_string(lang, device_desc, timeout)));
    }


    return Ok(());
}
