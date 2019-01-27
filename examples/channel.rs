extern crate hydro;

use hydro::task::{next, start};
use hydro::sync::mpsc::ReceiverExt;
use std::sync::mpsc;

extern "sysv64" fn go(recver: &mut mpsc::Receiver<String>) {
    for x in recver.hydro_iter() {
        println!("{}", x);
    }

    println!("all done");
}

fn main() {
    let (sender, mut recver) = mpsc::channel();
    start(move || go(&mut recver));
    let mut a = 12.0;
    for i in 0..10 {
        a += 13.7;
        sender.send(
            format!("hi there from iteration {}", i)
        ).unwrap();
        sender.send(
            format!("btw a = {}", a)
        ).unwrap();
        next();
    }

    drop(sender);
    next();
}
