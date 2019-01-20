#![feature(global_asm)]

#[macro_use]
extern crate lazy_static;

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

pub use crate::platform::{
    next,
    start,
    Task,
};

#[cfg(all(unix, target_arch = "x86_64"))]
mod platform {
    use crate::{
        align_mut_ptr_down,
        push_raw,
    };
    use std::cell::{Cell, RefCell};
    use std::collections::VecDeque;
    use std::ptr;
    use std::sync::Mutex;

    extern "sysv64" {
        fn start_inner(
            arg: *mut u8, // rdi
            rip: *const u8, // rsi
            rsp: *mut u8, // rdx
            save_rip: *mut *const u8, // rcx
            save_rsp: *mut *mut u8, // r8
            save_rbp: *mut *mut u8, // r9
        );

        fn pivot_inner(
            rip: *const u8, // rdi
            rsp: *mut u8, // rsi
            rbp: *mut u8, // rdx
            save_rip: *mut *const u8, // rcx
            save_rsp: *mut *mut u8, // r8
            save_rbp: *mut *mut u8, // r9
        );
    }

    global_asm!(r#"
        .intel_syntax
        .global start_inner
        .global pivot_inner

        start_inner:
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

            mov r11, rsi // new rip
            mov r12, rdx // new rsp

            lea rax, [rip+start_inner_back]
            mov [rcx], rax
            mov [r8], rsp
            mov [r9], rbp

            mov rsp, r12
            // same rdi
            jmp r11

        start_inner_back:
            ud2

        pivot_inner:
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

            lea rax, [rip+pivot_inner_back]
            mov [rcx], rax
            mov [r8], rsp
            mov [r9], rbp

            mov rsp, r12
            mov rbp, r13
            jmp r11

        pivot_inner_back:
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
    struct Context {
        rip: *const u8,
        rsp: *mut u8,
        rbp: *mut u8,
    }

    impl Context {
        pub(crate) fn null() -> Context {
            Context {
                rip: ptr::null(),
                rsp: ptr::null_mut(),
                rbp: ptr::null_mut(),
            }
        }

        pub(crate) unsafe fn pivot(&mut self, next: &Context) {
            let (save_rip, save_rsp, save_rbp) = (
                &mut self.rip as *mut *const u8,
                &mut self.rsp as *mut *mut u8,
                &mut self.rbp as *mut *mut u8,
            );
            assert!(is_x86_feature_detected!("avx")); // for vstmxcsr
            unsafe {
                pivot_inner(
                    next.rip,
                    next.rsp,
                    next.rbp,
                    save_rip,
                    save_rsp,
                    save_rbp,
                );
            }
        }
    }

    pub struct Task {
        stack: Vec<u8>,
        ctx: Option<Context>,
    }

/*
 *    impl Pool for PollingRoundRobinPool {
 *        fn remove(&mut self) {
 *            assert!(is_x86_feature_detected!("avx")); // for vstmxcsr
 *            if self.tasks.back().unwrap().stack.len() == 0 {
 *                panic!("can't remove main task");
 *            }
 *            if self.tasks.len() == 1 { // TODO: error, not panic
 *                panic!("can't remove only task");
 *            }
 *            assert_ne!(self.tasks.len(), 0);
 *
 *            let mut active = self.tasks.pop_back().unwrap();
 *
 *            {
 *                let t = self.tasks.pop_front().unwrap();
 *                self.tasks.push_back(t);
 *            }
 *
 *            unsafe {
 *                active.ctx.unwrap().pivot(
 *                    &self.tasks.back().unwrap().ctx.unwrap()
 *                );
 *            }
 *        }
 *    }
 */

    thread_local! {
        // back = active, front = next
        static tasks: RefCell<VecDeque<Task>> = RefCell::new(
            vec![Task { stack: vec![], ctx: None }].into()
        );
    }

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
        rsp = unsafe { rsp.offset(-8) };
        unsafe { ptr::write_bytes(rsp, 0, 8); } // "return address"

        let t = Task {
            stack,
            ctx: Some(Context {
                rip: f as *const u8,
                rsp,
                rbp: ptr::null_mut(),
            }),
        };

        // back = active, front = next

        tasks.with(|tt| {
            let mut tt = tt.borrow_mut();
            tt.push_front(t);
        });
        next();
    }

    pub fn next() {
        // back = active, front = next

        let ctxs = tasks.with(|tt| {
            let mut tt = tt.borrow_mut();

            if tt.len() == 1 {
                return None;
            }
            assert_ne!(tt.len(), 0);

            {
                let t = tt.pop_front().unwrap();
                tt.push_back(t);
            }

            let next_ctx = {
                let next_task = tt.back_mut().unwrap();
                next_task.ctx.take().unwrap()
            };

            let active_ctx_i = tt.len() - 2;
            tt[active_ctx_i].ctx = Some(Context::null());
            let active_ctx = unsafe {
                (
                    tt[active_ctx_i].ctx.as_mut().unwrap()
                    as *mut Context
                ).as_mut().unwrap()
            };

            // We stole active_ctx, so it *must not* survive past the end of
            // next(), and we *must not* modify the tasks vec.

            Some((active_ctx, next_ctx))
        });

        if let Some((active_ctx, next_ctx)) = ctxs {
            unsafe {
                active_ctx.pivot(&next_ctx);
            }
        }
    }
}
