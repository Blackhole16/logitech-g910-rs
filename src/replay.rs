use pcap::{Capture, Offline};
use std::path::Path;
use libusb::{DeviceHandle, Result as UsbResult, Error as UsbError, Context};
use usb::{Packet, TransferType, UrbType, Direction};
use std::time::Duration;
use std::thread;
use std::thread::JoinHandle;
use std::sync::mpsc::{channel, Sender, Receiver, TryRecvError};
use std::io;
use std::io::BufRead;
use utils;
use consts;

enum ReplayResponse {
    Success { buf: Vec<u8>, req_len: usize },
    Dropped,
    ThreadStarted,
    InProgress,
}

enum ReplayCompare {
    Correct,
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
}

impl<'a> Replay<'a> {
    pub fn replay_packet(&mut self, req: Packet) -> UsbResult<ReplayResponse> {
        if req.get_urb_type() != UrbType::Submit {
            return Err(UsbError::InvalidParam);
        }
        let timeout = Duration::from_secs(1);
        let mut buf = Vec::new();
        let read;
        // if new endpoint, detach kernel driver and claim interface
        if req.get_transfer_type() != TransferType::Interrupt {
            self.try_claim(req.get_endpoint()).unwrap();
        }
        // replay
        match req.get_transfer_type() {
            TransferType::Control => {
                buf.resize(req.get_w_length() as usize, 0u8);
                if req.get_direction() == Direction::In {
                    println!("Reading control packet...");
                    read = self.handle.read_control(
                            req.get_bm_request_type(),
                            req.get_b_request(),
                            req.get_value(),
                            req.get_language_id(),
                            &mut buf,
                            timeout
                    );
                } else {
                    println!("Writing control packet...");
                    read = self.handle.write_control(
                            req.get_bm_request_type(),
                            req.get_b_request(),
                            req.get_value(),
                            req.get_language_id(),
                            &buf,
                            timeout
                    );
                }
            },
            TransferType::Interrupt => {
                // if not already listening on that iface, start doing so
                if !self.listening.iter().any(|child| child.endpoint == req.get_endpoint()) {
                    println!("Writing interrupt packet...");
                    let tx = self.tx.clone();
                    let len = req.get_length() as usize;
                    let endpoint_direction = req.get_endpoint_direction();
                    let endpoint = req.get_endpoint();
                    // channel to send interrupt to
                    let (ltx, lrx) = channel();
                    let t = thread::spawn(move || read_interrupt(endpoint, endpoint_direction, len, lrx, tx));
                    self.listening.push(Child::new(t, ltx, req.get_endpoint_direction()));
                    return Ok(ReplayResponse::ThreadStarted);
                } else {
                    return Ok(ReplayResponse::InProgress);
                }
            }
            _ => unimplemented!()
        }
        match read {
            Ok(len) => if len == buf.len() {
                Ok(ReplayResponse::Success{ buf: buf, req_len: len })
            } else {
                buf.resize(len, 0u8);
                Ok(ReplayResponse::Success{ buf: buf, req_len: len })
            },
            Err(err) => Err(err)
        }
    }

    fn try_claim(&mut self, endpoint: u8) -> UsbResult<()> {
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
    pub fn new(path: &Path, context: &'a mut Context) -> Control<'a> {
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
            }
        }
    }

    pub fn skip(&mut self, count: u8) {
        for _ in 0..count {
            self.cap.next().unwrap();
        }
    }

    fn replay_next(&mut self) -> UsbResult<ReplayResponse> {
        let &mut Control { ref mut cap, ref mut replay } = self;
        let req = Packet::from_bytes(cap.next().unwrap().data).unwrap();
        if req.get_urb_type() != UrbType::Submit {
            println!("dropped (incorrect?) packet: {:?}", req);
            return Ok(ReplayResponse::Dropped);
        }
        replay.replay_packet(req)
    }

    fn compare_next(&mut self, res: UsbResult<ReplayResponse>) -> UsbResult<ReplayCompare> {
        let expected = Packet::from_bytes(self.cap.next().unwrap().data).unwrap();
        match res {
            Ok(replay_response) => {
                match replay_response {
                    ReplayResponse::Success{ buf, req_len } => {
                        let correct = expected.get_data() == &buf[..];
                        if req_len != buf.len() {
                            println!("Requested {} but only received {}", req_len, buf.len());
                        }
                        println!("Result correct: {}", correct);
                        if correct {
                            Ok(ReplayCompare::Correct)
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
            Err(err) => {
                let correct = expected.get_status() == err;
                println!("{}, expected {:?}, correct: {}", err, expected.get_status(), correct);
                if correct {
                    Ok(ReplayCompare::Correct)
                } else {
                    Err(err)
                }
            }
        }
    }

    pub fn replay_compare_next(&mut self) -> UsbResult<ReplayCompare> {
        // check for channel messages
        // TODO: move to own function
        while let Ok(res) = self.replay.rx.try_recv() {
            match res {
                Ok(buf) => {
                    println!("received {}: {:?}", buf.len(), buf);
                    let res = Packet::from_bytes(self.cap.next().unwrap().data).unwrap();
                    println!("Correct: {}", buf == res.get_data());
                },
                Err((endpoint, e)) => {
                    println!("Got error from child thread: {}", e);
                    self.replay.listening.retain(|child| endpoint != child.endpoint);
                }
            }
        }
        let res = self.replay_next();
        self.compare_next(res)
    }
    
    pub fn replay_all(&mut self) -> UsbResult<()> {
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
                Ok(ReplayCompare::Correct) => {},
                Ok(ReplayCompare::Dropped) => {},
                Ok(ReplayCompare::ThreadStarted) => {},
                Ok(ReplayCompare::InProgress) => {},
                Ok(ReplayCompare::Incorrect) => {
                    println!("Maybe incorrect???");
                    halt = true;
                },
                // real error during execution
                Err(e) => {
                    println!("Error replaying packet: {}", e);
                    halt = true;
                }
            }
        }
        Ok(())
    }
}

fn get_handle<'a>(context: &'a mut Context) -> DeviceHandle<'a> {
    match utils::open_device(
        context,
        &consts::VENDOR_ID,
        &consts::PRODUCT_ID
    ){
        Ok((_, _, handle)) => handle,
        Err(e) => {
            panic!("Could not get new DeviceHandle {}", e)
        }
    }
}

fn read_interrupt(endpoint: u8, endpoint_direction: u8, len: usize, lrx: Receiver<()>,
                  tx: Sender<Result<Vec<u8>, (u8, UsbError)>>) {
    let timeout = Duration::from_secs(1);
    let mut context = utils::get_context();
    let mut handle = get_handle(&mut context);
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
    match handle.claim_interface(endpoint) {
        Ok(_) => {},
        Err(e) => {
            println!("Could not claim interface {}", endpoint);
            tx.send(Err((endpoint, e))).unwrap();
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
    // cleanup
    handle.release_interface(endpoint).unwrap();
    handle.attach_kernel_driver(endpoint).unwrap();
}

