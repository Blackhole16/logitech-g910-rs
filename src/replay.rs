use pcap::{Capture, Offline};
use std::path::Path;
use libusb::{DeviceHandle, Result, Error};
use usb::{Packet, TransferType, UrbType};
use std::time::Duration;
use std::thread;
use std::sync::mpsc::{channel, Sender, Receiver, TryRecvError};
use utils;
use consts;

pub fn get_capture(path: &Path) -> Capture<Offline> {
    return Capture::from_file(path).unwrap();
}

pub fn replay(handle: &mut DeviceHandle, cap: &mut Capture<Offline>) -> Result<()> {
    let mut claimed = Vec::new();
    let mut listening = Vec::new();

    use std::io;
    use std::io::prelude::*;
    let timeout = Duration::from_secs(1);
    let stdin = io::stdin();
    let mut buf = Vec::new();
    let mut read;
    let (tx, rx) = channel();
    for _ in stdin.lock().lines() {
        // check for channel messages
        while let Ok((read, buf)) = rx.try_recv() {
            println!("received {}: {:?}", read, buf);
        }
        {
            let req = Packet::from_bytes(cap.next().unwrap().data).unwrap();
            if req.get_urb_type() != UrbType::Submit {
                println!("dropped (incorrect?) packet: {:?}", req);
                continue;
            }
            // if new endpoint, detach kernel driver and claim interface
            if req.get_transfer_type() != TransferType::Interrupt && !claimed.iter().any(|&(iface, _)| iface == req.get_endpoint()) {
                println!("Claiming interface {}", req.get_endpoint());
                // detch kernel driver
                let has_kernel_driver = utils::detach(handle, req.get_endpoint()).unwrap();
                try!(handle.claim_interface(req.get_endpoint()));
                claimed.push((req.get_endpoint(), has_kernel_driver));
            }
            // replay
            match req.get_transfer_type() {
                TransferType::Control => {
                    println!("Writing control packet...");
                    let mut bm_request_type = req.get_bm_request_type();
                    if bm_request_type & 0x80u8 != 0x80u8 {
                        println!("Corrected incorrect(?) Direction byte {}", bm_request_type);
                        bm_request_type |= 0x80u8;
                    }
                    buf.resize(req.get_w_length() as usize, 0u8);
                    read = handle.read_control(
                            bm_request_type,
                            req.get_b_request(),
                            req.get_value(),
                            req.get_language_id(),
                            &mut buf,
                            timeout
                    );
                },
                TransferType::Interrupt => {
                    println!("Writing interrupt packet...");
                    let tx = tx.clone();
                    let len = req.get_length() as usize;
                    let endpoint_direction = req.get_endpoint_direction();
                    let endpoint = req.get_endpoint();
                    // channel to send interrupt to
                    let (ltx, lrx) = channel();
                    let t = thread::spawn(move || read_interrupt(endpoint, endpoint_direction, len, lrx, tx));
                    listening.push((t, ltx, req.get_endpoint_direction()));
                    continue;
                }
                _ => unimplemented!()
            }
            match read {
                Ok(ref len) => if *len == buf.len() {
                    println!("Read {} bytes: {:?}", len, buf);
                } else {
                    println!("Read {} bytes, but requested {}: {:?}", len, buf.len(), &buf[..*len]);
                },
                Err(ref err) => println!("Error: {:?}", err)
            }
        }
        {
            let res = Packet::from_bytes(cap.next().unwrap().data).unwrap();
            match read {
                Ok(len) => println!("Result correct: {}", res.get_data() == &buf[..len]),
                Err(err) => println!("{}, expected {:?}, correct: {}", err, res.get_status(), res.get_status() == err)
            }
        }
    }

    // stop threads
    for &(_, ref ltx, _) in listening.iter() {
        // TODO: tell main-thread that a child-thread has died and remove from listening
        //   otherwise this unwrap fails
        ltx.send(()).unwrap();
    }
    for (t, _, _) in listening {
        t.join().unwrap();
    }

    for (iface, has_kernel_driver) in claimed {
        handle.release_interface(iface).unwrap();
        if has_kernel_driver {
            handle.attach_kernel_driver(iface).unwrap();
        }
    }
    Ok(())
}

fn read_interrupt(endpoint: u8, endpoint_direction: u8, len: usize, lrx: Receiver<()>, tx: Sender<(usize, Vec<u8>)>) {
    let timeout = Duration::from_secs(1);
    let mut context = utils::get_context();
    let (_,_,mut handle) = utils::open_device(
        &mut context,
        &consts::VENDOR_ID,
        &consts::PRODUCT_ID
    ).unwrap();
    println!("Claiming interface {}", endpoint);
    // detch kernel driver
    let has_kernel_driver = utils::detach(&mut handle, endpoint).unwrap();
    println!("had kernel driver: {}", has_kernel_driver);
    handle.claim_interface(0).unwrap();
    handle.claim_interface(endpoint).unwrap();
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
                println!("Read interrupt {} bytes: {:?}", len, buf);
                println!("sending to main...");
                tx.send((len, buf)).unwrap();
            },
            Err(Error::Timeout) => {},
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

