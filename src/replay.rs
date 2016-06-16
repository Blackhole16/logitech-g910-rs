use pcap::{Capture, Offline};
use std::path::Path;
use libusb::{DeviceHandle, Result as UsbResult, Error as UsbError, Context, AsyncGroup, Transfer};
use usb::{Packet, TransferType, UrbType, Direction};
use std::time::Duration;
use std::io;
use std::io::BufRead;
use std::u8;
use std::mem;
use color::{Color, KeyColor, ColorPacket, FlushPacket};
use keys::{StandardKey, GamingKey, KeyType};
use std::str::FromStr;

type SendResult = Result<SendResponse, SendResponseError>;
type RecvResult = UsbResult<Vec<u8>>;

#[derive(Debug, PartialEq)]
struct PacketInfo {
    req_len: usize,
    b_request: u8,
}

impl PacketInfo {
    fn new(req_len: usize, b_request: u8) -> PacketInfo {
        PacketInfo {
            req_len: req_len,
            b_request: b_request,
        }
    }
}

#[derive(Debug, PartialEq)]
enum SendResponse {
    Success { packet_info: PacketInfo },
    Dropped,
}
#[derive(Debug)]
enum SendResponseError {
    Error { packet_info: PacketInfo, err: UsbError },
    InvalidParam,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ReplayCompare {
    Correct(u8),
    ErrorExpected(u8),
    Dropped,
    Incorrect,
}

struct Replay<'a> {
    handle: &'a DeviceHandle<'a>,
    async_group: AsyncGroup<'a>,
    // TODO: use this flag
    handshake_done: bool,
    timeout: Duration,
}

impl<'a> Replay<'a> {
    pub fn send_control(&mut self, endpoint_direction: u8, buf: Vec<u8>, request_type: u8,
                        request: u8, value:u16, index: u16) ->  UsbResult<()> {
        println!("Initiating control packet...");
        self.async_group.submit(Transfer::control(
                self.handle,
                endpoint_direction,
                buf,
                request_type,
                request,
                value,
                index,
                self.timeout
        ))
    }

    pub fn send_interrupt(&mut self, endpoint_direction: u8, buf: Vec<u8>) -> UsbResult<()> {
        self.async_group.submit(Transfer::interrupt(self.handle, endpoint_direction, buf, self.timeout))
    }

    pub fn send_packet(&mut self, req: Packet) -> SendResult {
        if req.get_urb_type() != UrbType::Submit {
            return Err(SendResponseError::InvalidParam);
        }
        let mut buf = Vec::new();
        let len;
        let res = match req.get_transfer_type() {
            TransferType::Control => {
                if req.get_direction() == Direction::In {
                    buf.resize(req.get_w_length() as usize, 0u8);
                } else {
                    buf.extend_from_slice(req.get_data());
                }
                len = buf.len();
                println!("Initiating control packet...");
                self.send_control(
                        req.get_endpoint_direction(),
                        buf,
                        req.get_bm_request_type(),
                        req.get_b_request(),
                        req.get_value(),
                        req.get_language_id(),
                )
            },
            TransferType::Interrupt => {
                println!("Initiating interrupt packet on iface {}...",  req.get_endpoint());
                len = req.get_length() as usize;
                buf.resize(len, 0u8);
                let endpoint_direction = req.get_endpoint_direction();
                self.send_interrupt(endpoint_direction, buf)
            }
            _ => unimplemented!()
        };
        match res {
            Ok(_) => Ok(SendResponse::Success { packet_info: PacketInfo::new(len, req.get_b_request()) }),
            Err(err) => Err(SendResponseError::Error { packet_info:
                PacketInfo::new(len, req.get_b_request()), err: err })
        }
    }

    fn recv(&mut self) -> RecvResult {
        Ok(try!(self.async_group.wait_any()).actual().iter().cloned().collect())
    }

    fn send_color(&mut self, color_packet: ColorPacket) -> UsbResult<()> {
        let packet: [u8; 64] = unsafe { mem::transmute(color_packet) };
        let mut buf2 = Vec::new();
        buf2.resize(64, 0u8);
        try!(self.send_interrupt(0x82, buf2));

        let mut to_send = Vec::new();
        to_send.extend_from_slice(&packet);
        try!(self.send_control(0x80, to_send, 0x21, 9, 0x0212, 0x0001));
        match self.recv() {
            Ok(buf) => println!("OK: {:?}", &buf),
            Err(e) => println!("Err: {}", e)
        }
        match self.recv() {
            Ok(buf) => println!("OK: {:?}", &buf),
            Err(e) => println!("Err: {}", e)
        }
        Ok(())
    }
    fn flush_color(&mut self) -> UsbResult<()> {
        let flush: [u8; 20] = unsafe { mem::transmute(FlushPacket::new()) };
        let mut buf2 = Vec::new();
        buf2.resize(64, 0u8);
        try!(self.send_interrupt(0x82, buf2));

        let mut to_send = Vec::new();
        to_send.extend_from_slice(&flush);
        try!(self.send_control(0x80, to_send, 0x21, 9, 0x0212, 0x0001));
        match self.recv() {
            Ok(buf) => println!("OK: {:?}", &buf),
            Err(e) => println!("Err: {}", e)
        }
        match self.recv() {
            Ok(buf) => println!("OK: {:?}", &buf),
            Err(e) => println!("Err: {}", e)
        }
        Ok(())
    }
    fn set_color(&mut self, color_packet: ColorPacket) -> UsbResult<()> {
        try!(self.send_color(color_packet));
        try!(self.flush_color());
        Ok(())
    }
}

pub struct Control<'a> {
    cap: Capture<Offline>,
    replay: Replay<'a>,
}

impl<'a> Control<'a> {
    pub fn new(path: &Path, context: &'a Context, handle: &'a DeviceHandle<'a>) -> Control<'a> {
        Control {
            cap: Capture::from_file(path).unwrap(),
            replay: Replay{
                handle: handle,
                async_group: AsyncGroup::new(context),
                handshake_done: false,
                timeout: Duration::from_secs(10),
            }
        }
    }

    pub fn skip(&mut self, count: u8) {
        for _ in 0..count {
            self.cap.next().unwrap();
        }
    }

    fn send_next(&mut self) -> SendResult {
        let &mut Control { ref mut cap, ref mut replay } = self;
        let req = Packet::from_bytes(cap.next().unwrap().data).unwrap();
        if req.get_urb_type() != UrbType::Submit {
            println!("dropped (incorrect?) packet: {:?}", req);
            return Ok(SendResponse::Dropped);
        }
        replay.send_packet(req)
    }

    fn compare_next(&mut self, send: SendResult, recv: RecvResult) -> UsbResult<ReplayCompare> {
        let buf = try!(recv);
        match send {
            Ok(send_response) => {
                let expected = Packet::from_bytes(self.cap.next().unwrap().data).unwrap();
                match send_response {
                    SendResponse::Success { packet_info } => {
                        let PacketInfo { req_len, b_request } = packet_info;
                        let correct = expected.get_data() == &buf[..];
                        if req_len != buf.len() {
                            println!("Requested {} but only received {}", req_len, buf.len());
                        }
                        println!("Result correct: {}", correct);
                        if correct {
                            Ok(ReplayCompare::Correct(b_request))
                        } else {
                            println!("{:?}", expected.get_data());
                            println!("{:?}", buf);
                            Ok(ReplayCompare::Incorrect)
                        }
                    },
                    SendResponse::Dropped => {
                        println!("Packet dropped");
                        Ok(ReplayCompare::Dropped)
                    }
                }
            }
            Err(SendResponseError::Error { packet_info, err }) => {
                let expected = Packet::from_bytes(self.cap.next().unwrap().data).unwrap();
                let correct = expected.get_status() == err;
                println!("{}, expected {:?}, correct: {}", err, expected.get_status(), correct);
                if correct {
                    Ok(ReplayCompare::ErrorExpected(packet_info.b_request))
                } else {
                    Err(err)
                }
            },
            Err(SendResponseError::InvalidParam) => {
                println!("got invalid param");
                Err(UsbError::InvalidParam)
            }
        }
    }

    pub fn replay_compare_next(&mut self) -> UsbResult<ReplayCompare> {
        let send = self.send_next();
        let res = self.replay.recv();
        self.compare_next(send, res)
    }

    fn listen_iface2(&mut self) -> UsbResult<()> {
        let mut vec = Vec::new();
        vec.resize(64, 0u8);
        self.replay.send_interrupt(0x82, vec)
    }
    
    pub fn replay_stop<F>(&mut self, mut stop: F) -> UsbResult<()>
            where F: FnMut(&ReplayCompare) -> bool {
        let stdin = io::stdin();
        let mut halt = false;
        //for _ in stdin.lock().lines() {
        for i in 0.. {
            if halt {
                match stdin.lock().read_line(&mut String::new()) {
                    Ok(_) => {},
                    Err(_) => break
                }
                halt = false;
            }
            println!("{}:", i);
            match self.replay_compare_next() {
                Ok(ok) => {
                    if stop(&ok) {
                        return Ok(());
                    }
                    match ok {
                        ReplayCompare::Correct(_) => {},
                        ReplayCompare::ErrorExpected(_) => {},
                        ReplayCompare::Dropped => {},
                        ReplayCompare::Incorrect => {
                            println!("Maybe incorrect???");
                            halt = true;
                        },
                    }
                }
                // real error during execution
                Err(e) => {
                    println!("Error replaying packet: {}", e);
                    halt = true;
                }
            }
        }
        Ok(())
    }

    #[allow(unused)]
    pub fn replay_all(&mut self) -> UsbResult<()> {
        self.replay_stop(|_| false)
    }

    pub fn replay_basic_handshake(&mut self) -> UsbResult<()> {
        if self.replay.handshake_done {
            return Err(UsbError::InvalidParam);
        }
        self.replay.handshake_done = true;
        let mut i = 0;
        try!(self.replay_stop(move |p| {
            println!("{:?}", p);
            println!("i: {}", i);
            if *p != ReplayCompare::Correct(0x0a) {
                i += 1;
                if i >= 2 {
                    return true;
                }
            }
            false
        }));
        Ok(())
    }

    pub fn replay_handshake(&mut self) -> UsbResult<()> {
        try!(self.replay_basic_handshake());
        // GET DESCRIPTOR HID Report
        try!(self.replay_compare_next());
        // INTERRUP in iface 2
        let send2 = self.send_next();

        // continue replay until after the first SET_REPORT packet
        try!(self.replay_stop(|p| {
            println!("in fn: {:?}", p);
            *p == ReplayCompare::Correct(0x09)
        }));
        // next one should be the response on iface 2
        let recv = self.replay.recv();
        self.compare_next(send2, recv).unwrap();

        Ok(())
    }

    pub fn set_color(&mut self, key_color: KeyColor, key_type: KeyType) -> UsbResult<()> {
        let mut packet = ColorPacket::new(key_type);
        packet.add_key_color(key_color).unwrap();
        self.replay.set_color(packet)
    }

    pub fn set_all_colors(&mut self, color: Color) -> UsbResult<()> {
        for chunk in (&StandardKey::values()[..]).chunks(14) {
            let mut packet = ColorPacket::new_standard();
            for code in chunk {
                packet.add_key_color(KeyColor::new_standard(*code, color)).unwrap();
            }
            try!(self.replay.send_color(packet));
        }
        for chunk in (&GamingKey::values()[..]).chunks(14) {
            let mut packet = ColorPacket::new_gaming();
            for code in chunk {
                packet.add_key_color(KeyColor::new_gaming(*code, color)).unwrap();
            }
            try!(self.replay.send_color(packet));
        }
        self.replay.flush_color()
    }

    pub fn test(&mut self) -> UsbResult<()> {
        try!(self.replay_basic_handshake());
        self.replay.timeout = Duration::from_secs(1);
        try!(self.set_all_colors(Color::new(0,0,255)));
        //let streams = [
            //"11ff0f4b00040000000000000000000000000000",
            //"11ff0f4b00100000000000000000000000000000",
            //"11ff0f3b0010000202ff0000010000ff00000000",
            //"11ff0f4b00010000000000000000000000000000",
        //];

        // read colors from stdin and set them
        let stdin = io::stdin();
        println!("Reading colors... ");
        for l in stdin.lock().lines() {
            let l = l.unwrap();
            let split: Vec<_> = l.split(" ").collect();
            let mut key = StandardKey::from_str(split[0]).map(|s| s as u8);
            let mut key_type = KeyType::Standard;
            if let Err(_) = key {
                key = GamingKey::from_str(split[0]).map(|g| g as u8);
                key_type = KeyType::Gaming;
            }
            let r = u8::from_str(split[1]);
            let g = u8::from_str(split[2]);
            let b = u8::from_str(split[3]);
            match (key, r, g, b) {
                (Err(e), _, _, _) => println!("Could not parse key: {}", e),
                (_, Err(e), _, _) => println!("Could not parse red: {}", e),
                (_, _, Err(e), _) => println!("Could not parse green: {}", e),
                (_, _, _, Err(e)) => println!("Could not parse blue: {}", e),
                (Ok(k), Ok(r), Ok(g), Ok(b)) => try!(self.set_color(KeyColor::new(k, Color::new(r, g, b)), key_type))
            }
        }

        Ok(())
    }
} 

