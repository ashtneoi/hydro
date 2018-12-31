#![feature(global_asm)]

use std::is_x86_feature_detected;

// pub use platform::{};

#[cfg(all(unix, target_arch = "x86_64"))]
mod platform {
    use std::cell::Cell;

    thread_local! {
        static next_task: Cell<Option<Task>> = Cell::new(None);
    }

    global_asm!(r#"
        .intel_syntax

        pivot:
    "#);

    extern "sysv64" {
        fn pivot(
            rsp: *mut u8,
            f: extern "sysv64" fn(*const u8) -> !,
        );
    }

    struct Task {
        stack: Vec<u8>,
        rip: *const u8,
        rsp: *mut u8,
        rbp: *mut u8,
    }

    impl Task {
        fn start<T: Send>(
            f: extern "sysv64" fn(*const u8, *const u8) -> !,
            arg: T,
        ) {
            let mut stack = vec![0; 1<<18];
            let rsp_unaligned = stack.last_mut().unwrap() as *mut u8 as usize;
            // align to 512 bits
            let rsp = rsp_unaligned & 0xFFFF_FFFF_FFFF_FE00;
            let t = Task {
                stack: stack,
                rip: f as *const u8,
                rsp: rsp as *mut u8,
                rbp: rsp as *mut u8, // don't care
            };
        }
    }
}
