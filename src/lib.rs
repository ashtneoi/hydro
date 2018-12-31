#![feature(global_asm)]

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

pub use crate::platform::{next, start};

#[cfg(all(unix, target_arch = "x86_64"))]
mod platform {
    use crate::{
        align_mut_ptr_down,
        push_raw,
    };
    use std::cell::RefCell;
    use std::ptr;

    extern "sysv64" {
        fn start_raw(
            arg: *mut u8, // rdi
            rip: *const u8, // rsi
            rsp: *mut u8, // rdx
            next_rip: *mut *const u8, // rcx
            next_rsp: *mut *mut u8, // r8
            next_rbp: *mut *mut u8, // r9
        );
    }

    // TODO: how do we require GNU `as`?
    global_asm!(r#"
        .intel_syntax
        .global start_raw

        start_raw:
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

            mov r11, rsi # new rip
            mov r12, rdx # new rsp

            lea rax, [rip+start_raw_back]
            mov [rcx], rax
            mov [r8], rsp
            mov [r9], rbp

            mov rsp, r12
            # don't care about rbp
            push 0  # "return value"
            jmp r11  # TODO: far?

        start_raw_back:
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

            ret  # TODO: far?
    "#);

    extern "sysv64" {
        fn next_raw(
            rip: *const u8, // rdi
            rsp: *mut u8, // rsi
            rbp: *mut u8, // rdx
            next_rip: *mut *const u8, // rcx
            next_rsp: *mut *mut u8, // r8
            next_rbp: *mut *mut u8, // r9
        );
    }

    global_asm!(r#"
        .intel_syntax
        .global next_raw

        next_raw:
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

            mov r11, rdi // new rip
            mov r12, rsi // new rsp
            mov r13, rdx // new rbp

            lea rax, [rip+next_raw_back]
            mov [rcx], rax
            mov [r8], rsp
            mov [r9], rbp

            mov rsp, r12
            mov rbp, r13
            jmp r11

        next_raw_back:
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

            ret  # TODO: far?
    "#);

    #[derive(Clone, Copy)]
    pub struct Task {
        rip: *const u8,
        rsp: *mut u8,
        rbp: *mut u8,
    }

    impl Default for Task {
        fn default() -> Self {
            return Task {
                rip: ptr::null(),
                rsp: ptr::null_mut(),
                rbp: ptr::null_mut(),
            }
        }
    }

    thread_local! {
        static next_task: RefCell<Task> = RefCell::new(Default::default());
    }

    pub fn start<T: Send>(
        f: extern "sysv64" fn(&mut T) -> !,
        arg: T,
    ) -> Vec<u8> {
        assert!(is_x86_feature_detected!("avx")); // for vstmxcsr

        let mut stack = Vec::with_capacity(1<<18);
        unsafe { stack.set_len(1<<18); }
        let mut rsp = stack.last_mut().unwrap() as *mut u8;

        unsafe { push_raw(&mut rsp, arg); }
        let arg_ref = rsp;

        rsp = align_mut_ptr_down(rsp, 16);

        let t = Task {
            rip: f as *const u8,
            rsp: rsp as *mut u8,
            rbp: rsp as *mut u8, // don't care
        };

        next_task.with(|nt_rc| {
            let (nt_rip, nt_rsp, nt_rbp) = {
                let mut nt = nt_rc.borrow_mut();
                (
                    &mut nt.rip as *mut *const u8,
                    &mut nt.rsp as *mut *mut u8,
                    &mut nt.rbp as *mut *mut u8,
                )
            };
            unsafe {
                start_raw(
                    arg_ref,
                    t.rip,
                    t.rsp,
                    nt_rip,
                    nt_rsp,
                    nt_rbp,
                );
            }
        });

        return stack;
    }

    pub fn next() {
        next_task.with(|nt_rc| {
            let t = *nt_rc.borrow();
            let (nt_rip, nt_rsp, nt_rbp) = {
                let mut nt = nt_rc.borrow_mut();
                (
                    &mut nt.rip as *mut *const u8,
                    &mut nt.rsp as *mut *mut u8,
                    &mut nt.rbp as *mut *mut u8,
                )
            };
            unsafe {
                next_raw(
                    t.rip,
                    t.rsp,
                    t.rbp,
                    nt_rip,
                    nt_rsp,
                    nt_rbp,
                );
            }
        });
    }
}
