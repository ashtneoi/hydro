extern crate hydro;

use hydro::task::{next, start};
use std::net;

fn main() {
    let lst = net::TcpListener::bind("[::]:10000").unwrap();
    let (s, _) = lst.accept().unwrap();
}
