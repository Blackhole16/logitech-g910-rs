use pcap::{Capture, Offline};
use std::path::Path;
use libusb::{DeviceHandle, Result as UsbResult, Error as UsbError, Context, AsyncGroup, Transfer};
use usb::{Packet, TransferType, UrbType, Direction};
use std::time::Duration;
use std::io;
use std::io::BufRead;
use utils;
use consts;

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
    Claimed,
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum ReplayCompare {
    Correct(u8),
    ErrorExpected(u8),
    Dropped,
    Incorrect,
}

#[derive(Debug, Copy, Clone, PartialEq)]
struct Claim {
    endpoint: u8,
    has_kernel_driver: bool,
}

impl Claim {
    fn new(endpoint: u8, has_kernel_driver: bool) -> Claim {
        Claim {
            endpoint: endpoint,
            has_kernel_driver: has_kernel_driver,
        }
    }
}

struct Replay<'a> {
    handle: &'a DeviceHandle<'a>,
    async_group: AsyncGroup<'a>,
    // TODO: use this flag
    handshake_done: bool,
    claimed: Vec<Claim>,
    timeout: Duration,
}

impl<'a> Replay<'a> {
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
                self.async_group.submit::<'a>(Transfer::<'a>::control(
                        self.handle,
                        req.get_endpoint_direction(),
                        buf,
                        req.get_bm_request_type(),
                        req.get_b_request(),
                        req.get_value(),
                        req.get_language_id(),
                        self.timeout
                ))
            },
            TransferType::Interrupt => {
                println!("Reading interrupt packet...");
                len = req.get_length() as usize;
                buf.resize(len, 0u8);
                let endpoint = req.get_endpoint();
                let endpoint_direction = req.get_endpoint_direction();
                self.async_group.submit::<'a>(Transfer::<'a>::interrupt(self.handle, endpoint_direction, buf, self.timeout))
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
                claimed: Vec::new(),
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
        let expected = Packet::from_bytes(self.cap.next().unwrap().data).unwrap();
        let buf = try!(recv);
        match send {
            Ok(send_response) => {
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
            },
            Err(SendResponseError::Claimed) => {
                println!("device already claimed");
                Err(UsbError::InvalidParam)
            }
        }
    }

    pub fn replay_compare_next(&mut self) -> UsbResult<ReplayCompare> {
        let send = self.send_next();
        let mut res = self.replay.recv();
        self.compare_next(send, res)
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

    pub fn replay_handshake(&mut self) -> UsbResult<()> {
        if self.replay.handshake_done {
            return Err(UsbError::InvalidParam);
        }
        self.replay.handshake_done = true;
        let mut i = 0;
        try!(self.replay_stop(move |p| {
            println!("{:?}", p);
            println!("i: {}", i);
            match i {
                j if j < 2 && *p == ReplayCompare::Correct(0x0a) => {
                    i += 1;
                    false
                },
                i if i >= 2 => true,
                _ => false
            }
        }));
        // INTERRUP in iface 1
        self.send_next();
        // INTERRUP in iface 2
        self.send_next();
        self.skip(2);
        self.replay_stop(|p| {
            println!("{:?}", p);
            *p == ReplayCompare::Correct(0x09)
        })
    }

    //pub fn test(&mut self, context: &Context) -> UsbResult<()> {
        //try!(self.replay_handshake());
        //println!("handshake done");
        //println!("{:?}", self.replay.claimed);
        //println!("{:?}", self.replay.listening.iter().map(|c| c.endpoint).collect::<Vec<u8>>());
        ////self.replay.handle.release_interface(0);
        //self.replay.try_claim(1u8).unwrap();
        ////let t = thread::spawn(|| test2());
        //let mut buf1 = Vec::new();
        //buf1.resize(26, 0u8);
        //let mut buf2 = Vec::new();
        //buf2.resize(128, 0u8);
        //let mut buf3 = Vec::new();
        //buf3.resize(128, 0u8);
        //{
            //let mut async_group = AsyncGroup::new(&context);
            //println!("adding 0");
            //async_group.submit(Transfer::control(
                    //&self.replay.handle,
                    //0x80,
                    //buf1,
                    //0x80,
                    //0x06,
                    //0x0302,
                    //0x0409,
                    //Duration::from_secs(10)
            //)).unwrap();
            //println!("adding 1");
            //async_group.submit(Transfer::interrupt(&self.replay.handle, 0x81, buf2, Duration::from_secs(10))).unwrap();
            //println!("adding 2");
            //async_group.submit(Transfer::interrupt(&self.replay.handle, 0x82, buf3, Duration::from_secs(10))).unwrap();
            //loop {
                //println!("polling");
                //let mut transfer = async_group.wait_any().unwrap();
                //println!("{:?}, {:?}", transfer.status(), transfer.actual());
                //async_group.submit(transfer);
            //}
        //}
        //Ok(())
    //}
} 

