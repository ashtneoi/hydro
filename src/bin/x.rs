#![feature(asm)]

use hydrasync::{Context, thing};

pub fn main() {
    unsafe {
        let c = Context::call(thing);
    }
}
