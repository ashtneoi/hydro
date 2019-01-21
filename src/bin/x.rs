extern crate hydro;

use hydro::{next, start};

struct X;

impl Drop for X {
    fn drop(&mut self) {
        println!("oh noes i got dropped");
    }
}

extern "sysv64" fn go(arg: &mut X) {
    for _ in 0..8 {
        println!("go!");
        next();
    }
}

fn main() {
    start(go, X);
    let mut a = 12.0;
    if true {
        a += 13.7;
    }
    for _ in 0..10 {
        println!("a = {}", a);
        next();
    }
}
