use libusb::{
    Context,
    Device,
    DeviceDescriptor,
    LogLevel,
};

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

pub fn get_device(context: &mut Context, vendor_id: u16, product_id: u16) -> Option<Device> {
    for mut d in context.devices().unwrap().iter() {
        let dd = match d.device_descriptor() {
            Ok(dd) => dd,
            Err(_) => continue
        };
        if dd.product_id() == product_id && dd.vendor_id() == vendor_id {
            return Some(d);
        }
    }
    return None;
}
