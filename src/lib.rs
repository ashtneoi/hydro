#![feature(global_asm)]

use std::is_x86_feature_detected;
use std::mem::size_of;
use std::ptr;

pub(crate) fn align_mut_ptr_down<T>(p: *mut T, alignment: usize) -> *mut T {
    ((p as usize) & !((1<<alignment) - 1)) as *mut T
}

/// p need not be aligned.
pub(crate) unsafe fn push_raw<T>(p: &mut *mut u8, val: T) {
    *p = align_mut_ptr_down(*p, size_of::<T>());
    *p = ((*p as usize) - size_of::<T>()) as *mut u8;
    ptr::write(*p as *mut T, val);
}

pub use crate::platform::{Task};

#[cfg(all(unix, target_arch = "x86_64"))]
mod platform {
    use crate::{
        align_mut_ptr_down,
        push_raw,
    };
    use std::cell::Cell;

    thread_local! {
        static next_task: Cell<Option<Task>> = Cell::new(None);
    }

    // TODO: how do we require GNU `as`?
    global_asm!(r#"
        .intel_syntax

        pivot:
            # odd number of qwords for ABI
            push rbp
            push rbx
            push r12
            push r13
            push r14
            push r15
            push 0
            vstmxcsr [rsp]
            push 0
            fstcw [rsp]
            push 0
            # odd number of qwords for ABI

            mov r11, rsi # new rip
            mov r12, rdx # new rsp

            mov [rcx], rsp
            mov [r8], rbp
            lea rax, [rip+pivot_resume_b3c037d6b3912998]
            mov [r9], rax

            mov rsp, r12
            # don't care about rbp
            jmp r11

        pivot_resume_b3c037d6b3912998:
            fldcw [rsp]
            pop rax
            vldmxcsr [rsp]
            pop rax
            pop r15
            pop r14
            pop r13
            pop r12
            pop rbx
            pop rbp

            ret # TODO: far?
    "#);

    extern "sysv64" {
        fn pivot(
            arg: *mut u8, // rdi
            rip: *const u8, // rsi
            rsp: *mut u8, // rdx
            next_rip: &mut *const u8, // rcx
            next_rsp: &mut *mut u8, // r8
            next_rbp: &mut *mut u8, // r9
        );
    }

    pub struct Task {
        stack: Vec<u8>,
        rip: *const u8,
        rsp: *mut u8,
        rbp: *mut u8,
    }

    impl Task {
        pub fn start<T: Send>(
            f: extern "sysv64" fn(&mut T) -> !,
            arg: T,
        ) {
            assert!(is_x86_feature_detected!("avx")); // for vstmxcsr

            let mut stack = Vec::with_capacity(1<<18);
            unsafe { stack.set_len(1<<18); }
            let mut rsp = stack.last_mut().unwrap() as *mut u8;

            unsafe { push_raw(&mut rsp, arg); }
            let arg_ref = rsp;

            rsp = align_mut_ptr_down(rsp, 16);

            let t = Task {
                stack: stack,
                rip: f as *const u8,
                rsp: rsp as *mut u8,
                rbp: rsp as *mut u8, // don't care
            };

            next_task.with(|nt| {
                let mut nt2 = nt.take().unwrap();
                unsafe {
                    pivot(
                        arg_ref,
                        t.rip,
                        t.rsp,
                        &mut nt2.rip,
                        &mut nt2.rsp,
                        &mut nt2.rbp,
                    );
                }
                nt.set(Some(nt2));
            });
        }
    }
}
