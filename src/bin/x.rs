extern crate hydro;

use hydro::{next, start};
use std::sync::mpsc;

extern "sysv64" fn go(recver: &mut mpsc::Receiver<String>) {
    loop {
        match recver.try_recv() {
            Ok(x) => {
                println!("{}", x);
            },
            Err(mpsc::TryRecvError::Empty) => {
                next();
            },
            Err(mpsc::TryRecvError::Disconnected) => {
                break;
            },
        }
    }

    println!("all done");
}

fn main() {
    let (sender, recver) = mpsc::channel();
    start(go, recver);
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
