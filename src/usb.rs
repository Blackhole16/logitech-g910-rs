use std::mem;

#[derive(Debug)]
#[repr(C, packed)]
pub struct PacketBytes {
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

impl PacketBytes {
    pub fn from_bytes<'a>(bytes: &'a [u8]) -> Option<&'a PacketBytes> {
        if bytes.len() < 64 {
            return None;
        }
        Some(unsafe { mem::transmute(bytes.as_ptr()) })
    }
}

#[derive(Debug)]
pub struct Packet {
    id: u64,
    urb_type: UrbType,
    transfer_type: TransferType,
    direction: Direction,
    endpoint: u8,
    device: u8,
    bus_id: u16,
    setup_request: u8,
    data_present: bool,
    sec: u64,
    usec: u32,
    status: u32,
    length: u32,
    data_length: u32,
    interval: u32,
    start_frame: u32,
    transfer_flags: u32,
    num_iso_desc: u32,
}

impl Packet {
    pub fn from_bytes(bytes: &[u8]) -> Result<Self, &str> {
        if bytes.len() < 64 {
            return Err("Need at least 64 bytes to convert to PacketBytes");
        }
        return Ok(Packet {
            id: unsafe { *(&bytes[0] as *const u8 as *const u64) },
            urb_type: UrbType::from(bytes[8]),
            transfer_type: TransferType::from(bytes[9]),
            direction: Direction::from((bytes[10] & 0x80) == 0x80),
            endpoint: bytes[10] & 0x7f,
            device: bytes[11],
            bus_id: unsafe { *(bytes[12..14].as_ptr() as *const u16) },
            setup_request: bytes[14],
            data_present: bytes[15] == 0x00,
            sec: unsafe { *(bytes[16..24].as_ptr() as *const u64) },
            usec: unsafe { *(bytes[24..28].as_ptr() as *const u32) },
            status: unsafe { *(bytes[28..32].as_ptr() as *const u32) },
            length: unsafe { *(bytes[32..36].as_ptr() as *const u32) },
            data_length: unsafe { *(bytes[36..40].as_ptr() as *const u32) },
            // unused u64
            interval: unsafe { *(bytes[48..52].as_ptr() as *const u32) },
            start_frame: unsafe { *(bytes[52..56].as_ptr() as *const u32) },
            transfer_flags: unsafe { *(bytes[56..60].as_ptr() as *const u32) },
            num_iso_desc: unsafe { *(bytes[60..64].as_ptr() as *const u32) },
        });
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

