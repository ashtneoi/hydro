#![feature(asm)]

use hydro::Context;

extern "C" fn hi(c: Context) {
    println!("hi!");
    unsafe {
        c.activate(true);
    }
    panic!("OOF");
}

pub fn main() {
    unsafe {
        let mut c = Context::call(hi);
        loop {
            c = c.activate(true).unwrap();
        }
    }
}
