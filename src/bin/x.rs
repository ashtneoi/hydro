extern crate hydro;

use hydro::{next, start};
use std::process::abort;

extern "sysv64" fn go(arg: &mut u32) -> ! {
    loop {
        println!("arg = {}", *arg);
        next();
    }
}

fn main() {
    let v = start(go, 82);
    let mut a = 12.0;
    if true {
        a += 13.7;
    }
    loop {
        println!("a = {}", a);
        next();
    }
}
