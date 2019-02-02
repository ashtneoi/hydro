extern crate hydro;

use hydro::net::TcpListenerExt;
use hydro::task::{next, start};
use std::net;
use std::thread::sleep;
use std::time::Duration;

fn main() {
    start(move || {
        println!("10000 started");
        let lst = net::TcpListener::bind("[::]:10000").unwrap();
        lst.set_nonblocking(true).unwrap();
        for _ in lst.hydro_incoming() {
            println!("10000");
        }
    });
    start(move || {
        println!("10001 started");
        let lst = net::TcpListener::bind("[::]:10001").unwrap();
        lst.set_nonblocking(true).unwrap();
        for _ in lst.hydro_incoming() {
            println!("10001");
        }
    });
    loop {
        next();
        sleep(Duration::from_millis(100));
    }
}
