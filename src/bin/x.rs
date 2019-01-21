extern crate hydro;

use hydro::{next, start};

extern "sysv64" fn go(arg: &mut u32) {
    for _ in 0..8 {
        println!("arg = {}", *arg);
        next();
    }
}

fn main() {
    start(go, 82);
    let mut a = 12.0;
    if true {
        a += 13.7;
    }
    for _ in 0..10 {
        println!("a = {}", a);
        next();
    }
}
