extern crate hydro;

use hydro::task::{next, start};

fn main() {
    start(move || {
        println!("hi!");
    });
    next();
}
