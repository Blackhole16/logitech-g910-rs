use pcap::{Capture, Offline};
use std::path::Path;
use libusb::{DeviceHandle, Result as UsbResult, Error as UsbError, Context, AsyncGroup, Transfer};
use usb::{Packet, TransferType, UrbType, Direction};
use std::time::Duration;
use std::thread;
use std::thread::JoinHandle;
use std::sync::mpsc::{channel, Sender, Receiver, TryRecvError};
use std::io;
use std::io::BufRead;
use utils;
use consts;

type ReplayResult = Result<ReplayResponse, ReplayResponseError>;

#[derive(Debug, PartialEq)]
struct PacketInfo {
    buf: Option<Vec<u8>>,
    req_len: usize,
    b_request: u8,
}

impl PacketInfo {
    fn new(buf: Option<Vec<u8>>, req_len: usize, b_request: u8) -> PacketInfo {
        PacketInfo {
            buf: buf,
            req_len: req_len,
            b_request: b_request,
        }
    }
}

#[derive(Debug, PartialEq)]
enum ReplayResponse {
    Success { packet_info: PacketInfo },
    Dropped,
    ThreadStarted,
    InProgress,
}
#[derive(Debug)]
enum ReplayResponseError {
    Error { packet_info: PacketInfo, err: UsbError },
    InvalidParam,
    Claimed,
}

#[derive(Debug, PartialEq)]
pub enum ReplayCompare {
    Correct(u8),
    ErrorExpected(u8),
    Dropped,
    Incorrect,
    ThreadStarted,
    InProgress,
}

struct Child {
    endpoint: u8,
    thread: JoinHandle<()>,
    tx: Sender<()>,
}

impl Child {
    fn new(thread: JoinHandle<()>, tx: Sender<()>, endpoint: u8) -> Child {
        Child {
            endpoint: endpoint,
            thread: thread,
            tx: tx,
        }
    }
}

#[derive(Debug)]
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
    handle: DeviceHandle<'a>,
    // TODO: use this flag
    handshake_done: bool,
    listening: Vec<Child>,
    claimed: Vec<Claim>,
    tx: Sender<Result<Vec<u8>, (u8, UsbError)>>,
    rx: Receiver<Result<Vec<u8>, (u8, UsbError)>>,
    timeout: Duration,
}

impl<'a> Replay<'a> {
    pub fn read_control(&mut self, buf: &mut Vec<u8>, endpoint: u8,
            bm_request_type: u8, b_request: u8, value: u16, language_id: u16) -> UsbResult<usize> {
        // if new endpoint, detach kernel driver and claim interface
        self.try_claim(endpoint).unwrap();
        self.handle.read_control(
                bm_request_type,
                b_request,
                value,
                language_id,
                buf,
                self.timeout
        )
    }
    pub fn write_control(&mut self, buf: &Vec<u8>, endpoint: u8,
            bm_request_type: u8, b_request: u8, value: u16, language_id: u16) -> UsbResult<usize> {
        // if new endpoint, detach kernel driver and claim interface
        self.try_claim(endpoint).unwrap();
        self.handle.write_control(
                bm_request_type,
                b_request,
                value,
                language_id,
                buf,
                self.timeout
        )
    }
    pub fn read_interrupt(&mut self, endpoint: u8, endpoint_direction: u8, len: usize) -> ReplayResult {
        // if a thread is already listening on that interface, return error
        if self.listening.iter().any(|child| child.endpoint == endpoint) {
            return Ok(ReplayResponse::InProgress);
        }
        // otherwise claim interface if not already claimed
        self.try_claim(endpoint).unwrap();
        let mut buf: Vec<u8> = (0..len).map(|_| 0u8).collect();
        match self.handle.read_interrupt(endpoint_direction, &mut buf, self.timeout) {
            Ok(read) => buf.resize(read, 0u8),
            Err(err) => return Err(ReplayResponseError::Error {
                packet_info: PacketInfo::new(None, len, 0u8), err: err})
        }
        return Ok(ReplayResponse::Success { packet_info: PacketInfo::new(Some(buf), len, 0u8) });
    }
    pub fn read_interrupt_async(&mut self, endpoint: u8, endpoint_direction: u8, len: usize)
            -> ReplayResult {
        // if not already listening on that iface, start doing so
        if self.listening.iter().any(|child| child.endpoint == endpoint) {
            return Ok(ReplayResponse::InProgress);
        }
        if self.claimed.iter().any(|claimed| claimed.endpoint == endpoint) {
            return Err(ReplayResponseError::Claimed);
        }
        let tx = self.tx.clone();
        // channel to send interrupt to
        let (ltx, lrx) = channel();
        let t = thread::spawn(move || read_interrupt(endpoint, endpoint_direction, len, lrx, tx));
        self.listening.push(Child::new(t, ltx, endpoint_direction));
        return Ok(ReplayResponse::ThreadStarted);
    }

    pub fn replay_packet(&mut self, req: Packet) -> ReplayResult {
        if req.get_urb_type() != UrbType::Submit {
            return Err(ReplayResponseError::InvalidParam);
        }
        let mut buf = Vec::new();
        let read;
        // replay
        match req.get_transfer_type() {
            TransferType::Control => {
                buf.resize(req.get_w_length() as usize, 0u8);
                if req.get_direction() == Direction::In {
                    println!("Reading control packet...");
                    read = self.read_control(
                            &mut buf,
                            req.get_endpoint(),
                            req.get_bm_request_type(),
                            req.get_b_request(),
                            req.get_value(),
                            req.get_language_id(),
                    );
                } else {
                    println!("Writing control packet...");
                    read = self.write_control(
                            &buf,
                            req.get_endpoint(),
                            req.get_bm_request_type(),
                            req.get_b_request(),
                            req.get_value(),
                            req.get_language_id(),
                    );
                }
                match read {
                    Ok(len) => if len == buf.len() {
                        Ok(ReplayResponse::Success { packet_info: PacketInfo::new(Some(buf), len, req.get_b_request()) })
                    } else {
                        buf.resize(len, 0u8);
                        Ok(ReplayResponse::Success { packet_info: PacketInfo::new(Some(buf), len, req.get_b_request()) })
                    },
                    Err(err) => Err(ReplayResponseError::Error { packet_info:
                        PacketInfo::new(None, 0, req.get_b_request()), err: err })
                }
            },
            TransferType::Interrupt => {
                println!("Reading interrupt packet...");
                let len = req.get_length() as usize;
                let endpoint = req.get_endpoint();
                let endpoint_direction = req.get_endpoint_direction();
                // channel to send interrupt to
                self.read_interrupt_async(endpoint, endpoint_direction, len)
            }
            _ => unimplemented!()
        }
    }

    fn try_claim(&mut self, mut endpoint: u8) -> UsbResult<()> {
        // for some reason we cannot claim interface 2 as it doesn't exist
        // but we are able to read from it, if we claim interface 1
        // yep, i love logitech
        if endpoint == 2 {
            endpoint = 1;
        }
        // if new endpoint, detach kernel driver and claim interface
        if !self.claimed.iter().any(|claim| claim.endpoint == endpoint) {
            println!("Claiming interface {}", endpoint);
            // detch kernel driver
            let has_kernel_driver = utils::detach(&mut self.handle, endpoint).unwrap();
            try!(self.handle.claim_interface(endpoint));
            self.claimed.push(Claim::new(endpoint, has_kernel_driver));
        }
        Ok(())
    }
}

impl<'a> Drop for Replay<'a> {
    fn drop(&mut self) {
        // stop threads
        for child in self.listening.iter() {
            // TODO: tell main-thread that a child-thread has died and remove from listening
            //   otherwise this unwrap fails
            child.tx.send(()).unwrap();
        }
        for child in self.listening.drain(..) {
            child.thread.join().unwrap();
        }

        for claim in self.claimed.iter() {
            self.handle.release_interface(claim.endpoint).unwrap();
            if claim.has_kernel_driver {
                self.handle.attach_kernel_driver(claim.endpoint).unwrap();
            }
        }
    }
}

pub struct Control<'a> {
    cap: Capture<Offline>,
    replay: Replay<'a>,
}

impl<'a> Control<'a> {
    pub fn new(path: &Path, context: &'a Context) -> Control<'a> {
        let (tx, rx) = channel();
        Control {
            cap: Capture::from_file(path).unwrap(),
            replay: Replay{
                handle: get_handle(context),
                handshake_done: false,
                listening: Vec::new(),
                claimed: Vec::new(),
                tx: tx,
                rx: rx,
                timeout: Duration::from_secs(1),
            }
        }
    }

    pub fn skip(&mut self, count: u8) {
        for _ in 0..count {
            self.cap.next().unwrap();
        }
    }

    fn replay_next(&mut self) -> ReplayResult {
        let &mut Control { ref mut cap, ref mut replay } = self;
        let req = Packet::from_bytes(cap.next().unwrap().data).unwrap();
        if req.get_urb_type() != UrbType::Submit {
            println!("dropped (incorrect?) packet: {:?}", req);
            return Ok(ReplayResponse::Dropped);
        }
        replay.replay_packet(req)
    }

    fn compare_next(&mut self, res: ReplayResult) -> UsbResult<ReplayCompare> {
        let expected = Packet::from_bytes(self.cap.next().unwrap().data).unwrap();
        match res {
            Ok(replay_response) => {
                match replay_response {
                    ReplayResponse::Success { packet_info } => {
                        let PacketInfo { buf, req_len, b_request } = packet_info;
                        let buf = buf.unwrap();
                        let correct = expected.get_data() == &buf[..];
                        if req_len != buf.len() {
                            println!("Requested {} but only received {}", req_len, buf.len());
                        }
                        println!("Result correct: {}", correct);
                        if correct {
                            Ok(ReplayCompare::Correct(b_request))
                        } else {
                            Ok(ReplayCompare::Incorrect)
                        }
                    },
                    ReplayResponse::Dropped => {
                        println!("Packet dropped");
                        Ok(ReplayCompare::Dropped)
                    }
                    ReplayResponse::ThreadStarted => {
                        println!("Started new Thread listening on that device");
                        Ok(ReplayCompare::ThreadStarted)
                    }
                    ReplayResponse::InProgress => {
                        println!("Already listening on that device");
                        Ok(ReplayCompare::InProgress)
                    }
                }
            }
            Err(ReplayResponseError::Error { packet_info, err }) => {
                let correct = expected.get_status() == err;
                println!("{}, expected {:?}, correct: {}", err, expected.get_status(), correct);
                if correct {
                    Ok(ReplayCompare::ErrorExpected(packet_info.b_request))
                } else {
                    Err(err)
                }
            },
            Err(ReplayResponseError::InvalidParam) => {
                println!("got invalid param");
                Err(UsbError::InvalidParam)
            },
            Err(ReplayResponseError::Claimed) => {
                println!("device already claimed");
                Err(UsbError::InvalidParam)
            }
        }
    }

    pub fn try_poll(&mut self) -> Option<Result<Vec<u8>, (u8, UsbError)>> {
        match self.replay.rx.try_recv() {
            Ok(res) => match res {
                Ok(buf) => {
                    Some(Ok(buf))
                },
                Err((endpoint, e)) => {
                    println!("Got error from child thread: {}", e);
                    self.replay.listening.retain(|child| endpoint != child.endpoint);
                    Some(Err((endpoint, e)))
                }
            },
            Err(_) => None
        }
    }

    pub fn poll_compare(&mut self) {
        while let Some(res) = self.try_poll() {
            match res {
                Ok(buf) => {
                    println!("received {}: {:?}", buf.len(), buf);
                    let res = Packet::from_bytes(self.cap.next().unwrap().data).unwrap();
                    println!("Correct: {}", buf == res.get_data());
                },
                Err((endpoint, e)) => {}
            }
        }
        
    }

    pub fn replay_compare_next(&mut self) -> UsbResult<ReplayCompare> {
        // check for channel messages
        let res = self.replay_next();
        self.compare_next(res)
    }
    
    pub fn replay_stop<F>(&mut self, stop: F) -> UsbResult<()>
            where F: Fn(&ReplayCompare) -> bool {
        let stdin = io::stdin();
        let mut halt = false;
        //for _ in stdin.lock().lines() {
        for i in 0.. {
            self.poll_compare();
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
                        ReplayCompare::ThreadStarted => {},
                        ReplayCompare::InProgress => {},
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
        self.replay_stop(|p| {
            println!("{:?}", p);
            *p == ReplayCompare::ErrorExpected(0x0a)
        })
    }

    pub fn test(&mut self, context: &Context) -> UsbResult<()> {
        try!(self.replay_handshake());
        println!("handshake done");
        println!("{:?}", self.replay.claimed);
        println!("{:?}", self.replay.listening.iter().map(|c| c.endpoint).collect::<Vec<u8>>());
        //self.replay.handle.release_interface(0);
        self.replay.try_claim(1u8).unwrap();
        //let t = thread::spawn(|| test2());
        let mut buf1 = Vec::new();
        buf1.resize(26, 0u8);
        let mut buf2 = Vec::new();
        buf2.resize(128, 0u8);
        let mut buf3 = Vec::new();
        buf3.resize(128, 0u8);
        {
            let mut async_group = AsyncGroup::new(&context);
            println!("adding 0");
            async_group.submit(Transfer::control(
                    &self.replay.handle,
                    0x80,
                    &mut buf1,
                    0x80,
                    0x06,
                    0x0302,
                    0x0409,
                    Duration::from_secs(10)
            )).unwrap();
            println!("adding 1");
            async_group.submit(Transfer::interrupt(&self.replay.handle, 0x81, &mut buf2, Duration::from_secs(10))).unwrap();
            println!("adding 2");
            async_group.submit(Transfer::interrupt(&self.replay.handle, 0x82, &mut buf3, Duration::from_secs(10))).unwrap();
            loop {
                println!("polling");
                let mut transfer = async_group.wait_any().unwrap();
                println!("{:?}, {:?}", transfer.status(), transfer.actual());
                async_group.submit(transfer);
            }
        }
        Ok(())
    }
} 

fn get_handle(context: &Context) -> DeviceHandle {
    match utils::open_device(
        context,
        consts::VENDOR_ID,
        consts::PRODUCT_ID
    ){
        Ok((_, _, handle)) => handle,
        Err(e) => {
            panic!("Could not get new DeviceHandle {}", e)
        }
    }
}

fn read_interrupt(endpoint_given: u8, endpoint_direction: u8, len: usize, lrx: Receiver<()>,
                  tx: Sender<Result<Vec<u8>, (u8, UsbError)>>) {
    let timeout = Duration::from_secs(10);
    let mut context = utils::get_context();
    let mut handle = get_handle(&mut context);
    // we all love logitech
    let endpoint;
    if endpoint_given == 2 {
        endpoint = 1;
    } else {
        endpoint = endpoint_given;
    }
    println!("Claiming interface {}", endpoint);
    // detch kernel driver
    let has_kernel_driver = match utils::detach(&mut handle, endpoint) {
        Ok(b) => b,
        Err(e) => {
            println!("Could not detach kernel driver for {}", endpoint);
            tx.send(Err((endpoint, e))).unwrap();
            return;
        }
    };
    println!("had kernel driver: {}", has_kernel_driver);
    let is_claimed;
    match handle.claim_interface(endpoint) {
        Ok(_) => is_claimed = true,
        Err(e) => {
            println!("Could not claim interface {}", endpoint);
            //tx.send(Err((endpoint, e))).unwrap();
            is_claimed = false;
        }
    }
    loop {
        // check if need to stop
        match lrx.try_recv() {
            Ok(_) => break,
            Err(TryRecvError::Empty) => {},
            Err(TryRecvError::Disconnected) => break
        }
        let mut buf = Vec::new();
        buf.resize(len, 0u8);
        match handle.read_interrupt(
                endpoint_direction,
                &mut buf,
                timeout
        ){
            Ok(len) => {
                buf.resize(len, 0u8);
                println!("Read interrupt {} bytes: {:?}", len, buf);
                println!("sending to main...");
                tx.send(Ok(buf)).unwrap();
            },
            Err(UsbError::Timeout) => {},
            Err(err) => {
                println!("Err reading interrupt: {}", err);
                break;
            }
        }
    }
    println!("thread for endpoint {} cleaning up", endpoint_given);
    // cleanup
    if is_claimed {
        handle.release_interface(endpoint).unwrap();
    }
    if has_kernel_driver {
        handle.attach_kernel_driver(endpoint).unwrap();
    }
}

