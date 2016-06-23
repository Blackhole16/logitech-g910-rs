#![feature(inclusive_range_syntax)]

extern crate libusb;
extern crate pcap;
extern crate g910;

mod print;
mod replay;
mod usb;
mod test;

use std::path::Path;
use replay::Control;

use g910::{Keyboard, Color, KeyEvent};

fn main() {
    test::print_memory_layout();
    //return;
    //let p = Path::new("pcap/g910/handshake/handshake2.pcap");
    //test::print_all_data(&p);
    //return;

    //let p1 = Path::new("pcap/g910/color/space-red.pcap");
    //let p2 = Path::new("pcap/g910/color/space-blue.pcap");
    //test::compare(&p1, &p2);
    //return;
    
    let context = g910::get_context();
    let mut handle = g910::get_handle(&context).unwrap();
    let mut keyboard = Keyboard::new(&context, &*handle).unwrap();
    keyboard.set_all_colors(Color::new(255, 0, 0));
    keyboard.handle(|evt, keyboard| {
        match evt {
            KeyEvent::KeyPressed(k) => {
                println!("Key pressed: {:?}", k);
                keyboard.set_all_colors(Color::new(255, 0, 0));
            },
            KeyEvent::KeyReleased(k) => {
                println!("Key released: {:?}", k);
                keyboard.set_all_colors(Color::new(0, 0, 255));
            },
        }
        return true;
    });
    return;


    let p = Path::new("pcap/g910/handshake/handshake.pcap");

    {
        let mut ctrl = Control::new(&p, &context, &*handle);
        // first 6 packets are from wireshark
        ctrl.skip(6);
        ctrl.test().unwrap();
        ctrl.replay_handshake().unwrap();
    }
}

