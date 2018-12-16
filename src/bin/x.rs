#![feature(asm)]

use hydrasync::{Context, thing};

pub fn main() {
    unsafe {
        let mut c = Context::call(thing);
        loop {
            c = c.activate();
        }
    }
}
