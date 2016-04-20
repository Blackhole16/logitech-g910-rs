use std::mem;

#[derive(Debug)]
#[repr(C, packed)]
pub struct Packet {
    id: u64,
    urb_type: u8,
    transfer_type: u8,
    endpoint_direction: u8,
    device: u8,
    bus_id: u16,
    setup_request: u8,
    data_present: u8,
    sec: u64,
    usec: u32,
    status: u32,
    length: u32,
    data_length: u32,
    unused: u64,
    interval: u32,
    start_frame: u32,
    transfer_flags: u32,
    num_iso_desc: u32,
}

impl Packet {
    pub fn from_bytes<'a>(bytes: &'a [u8]) -> Option<&'a Packet> {
        if bytes.len() < 64 {
            return None;
        }
        Some(unsafe { mem::transmute(bytes.as_ptr()) })
    }

    pub fn get_id(&self) -> u64 {
        self.id
    }
    pub fn get_urb_type(&self) -> UrbType {
        UrbType::from(self.urb_type)
    }
    pub fn getTransferType(&self) -> TransferType {
        TransferType::from(self.transfer_type)
    }
    pub fn getDirection(&self) -> Direction {
        Direction::from((self.endpoint_direction & 0x80) == 0x80)
    }
    pub fn get_endpoint(&self) -> u8 {
        self.endpoint_direction & 0x7f
    }
    pub fn get_device(&self) -> u8 {
        self.device
    }
    pub fn get_bus_id(&self) -> u16 {
        self.bus_id
    }
    pub fn get_setup_request(&self) -> u8 {
        self.setup_request
    }
    pub fn is_data_present(&self) -> bool {
        self.data_present == 0x00
    }
    pub fn get_sec(&self) -> u64 {
        self.sec
    }
    pub fn get_usec(&self) -> u32 {
        self.usec
    }
    pub fn get_status(&self) -> u32 {
        self.status
    }
    pub fn get_length(&self) -> u32 {
        self.length
    }
    pub fn get_data_length(&self) -> u32 {
        self.data_length
    }
    pub fn get_interval(&self) -> u32 {
        self.interval
    }
    pub fn get_start_frame(&self) -> u32 {
        self.start_frame
    }
    pub fn get_transfer_flags(&self) -> u32 {
        self.transfer_flags
    }
    pub fn get_num_iso_desc(&self) -> u32 {
        self.num_iso_desc
    }
}

#[derive(Debug)]
pub enum UrbType {
    Submit, Complete
}

impl From<u8> for UrbType {
    fn from(byte: u8) -> Self {
        return match byte {
            0x43u8 => UrbType::Complete,
            0x53u8 => UrbType::Submit,
            // TODO: std::convert::From must not fail
            // greetings from https://github.com/rust-lang/rfcs/pull/1542
            _ => panic!("Unknown UrbType {}", byte),
        }
    }
}

#[derive(Debug)]
pub enum TransferType {
    Control, Isochronous, Bulk, Interrupt
}

impl From<u8> for TransferType {
    fn from(byte: u8) -> Self {
        return match byte {
            0x01 => TransferType::Interrupt,
            0x02 => TransferType::Control,
            // TODO: std::convert::From must not fail
            // greetings from https://github.com/rust-lang/rfcs/pull/1542
            _ => panic!("Unknown TransferType {}", byte),
        }
    }
}

#[derive(Debug)]
pub enum Direction {
    In, Out
}

impl From<bool> for Direction {
    fn from(b: bool) -> Self {
        return match b {
            true => Direction::In,
            false => Direction::Out
        }
    }
}

