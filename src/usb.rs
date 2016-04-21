use std::mem;

#[derive(Debug)]
#[repr(C, packed)]
struct PacketHead {
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
    bm_request_type: u8,
    b_request: u8,
    descriptor_index: u8,
    descriptor_type: u8,
    language_id: u16,
    w_length: u16,
    interval: u32,
    start_frame: u32,
    transfer_flags: u32,
    num_iso_desc: u32,
}

#[derive(Debug)]
pub struct Packet<'a> {
    head: &'a PacketHead,
    data: &'a [u8],
}

impl<'a> Packet<'a> {
    pub fn from_bytes(bytes: &'a [u8]) -> Option<Packet> {
        if bytes.len() < 64 {
            return None;
        }
        let head = unsafe { mem::transmute(bytes[..64].as_ptr()) };
        Some(Packet { head: head, data: &bytes[64..] })
    }

    pub fn get_id(&self) -> u64 {
        self.head.id
    }
    pub fn get_urb_type(&self) -> UrbType {
        UrbType::from(self.head.urb_type)
    }
    pub fn get_transfer_type(&self) -> TransferType {
        TransferType::from(self.head.transfer_type)
    }
    pub fn get_direction(&self) -> Direction {
        Direction::from((self.head.endpoint_direction & 0x80) == 0x80)
    }
    pub fn get_endpoint(&self) -> u8 {
        self.head.endpoint_direction & 0x7f
    }
    pub fn get_device(&self) -> u8 {
        self.head.device
    }
    pub fn get_bus_id(&self) -> u16 {
        self.head.bus_id
    }
    pub fn get_setup_request(&self) -> u8 {
        self.head.setup_request
    }
    pub fn is_data_present(&self) -> bool {
        // yep, you read correctly!
        // if data is present, the value is actually 0x00
        self.head.data_present == 0x00
    }
    pub fn get_sec(&self) -> u64 {
        self.head.sec
    }
    pub fn get_usec(&self) -> u32 {
        self.head.usec
    }
    pub fn get_status(&self) -> u32 {
        self.head.status
    }
    pub fn get_length(&self) -> u32 {
        self.head.length
    }
    pub fn get_data_length(&self) -> u32 {
        self.head.data_length
    }
    pub fn get_bm_request_type(&self) -> u8 {
        self.head.bm_request_type
    }
    pub fn get_b_request(&self) -> u8 {
        self.head.b_request
    }
    pub fn get_descriptor_index(&self) -> u8 {
        self.head.descriptor_index
    }
    pub fn get_descriptor_type(&self) -> u8 {
        self.head.descriptor_type
    }
    pub fn get_value(&self) -> u16 {
        (self.head.descriptor_type as u16) << 8 | self.head.descriptor_index as u16
    }
    pub fn get_language_id(&self) -> u16 {
        self.head.language_id
    }
    pub fn get_w_length(&self) -> u16 {
        self.head.w_length
    }
    pub fn get_interval(&self) -> u32 {
        self.head.interval
    }
    pub fn get_start_frame(&self) -> u32 {
        self.head.start_frame
    }
    pub fn get_transfer_flags(&self) -> u32 {
        self.head.transfer_flags
    }
    pub fn get_num_iso_desc(&self) -> u32 {
        self.head.num_iso_desc
    }
    pub fn get_data(&self) -> &[u8] {
        self.data
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

