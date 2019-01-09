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

pub use crate::platform::{
    next,
    PollingRoundRobinPool,
    Pool,
    start,
    Task,
};

#[cfg(all(unix, target_arch = "x86_64"))]
mod platform {
    use crate::{
        align_mut_ptr_down,
        push_raw,
    };
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::ptr;

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
    pub struct Context {
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

    pub trait Pool {
        fn add(&mut self, t: Task);
        fn next(&mut self);
        fn remove(&mut self);
    }

    pub struct PollingRoundRobinPool {
        tasks: VecDeque<Task>, // back = current, front = next
    }

    impl PollingRoundRobinPool {
        pub fn activate_new() {
            let mut tasks = VecDeque::with_capacity(32);
            tasks.push_back(Task {
                stack: Vec::new(),
                ctx: None,
            });

            active_pool.with(|ap| {
                ap.replace(
                    Some(Box::new(Self { tasks }))
                );
            });
        }
    }

    impl Pool for PollingRoundRobinPool {
        // back is active, front is next
        fn next(&mut self) {
            if self.tasks.len() == 1 {
                return;
            }
            assert_ne!(self.tasks.len(), 0);

            {
                let t = self.tasks.pop_front().unwrap();
                self.tasks.push_back(t);
            }

            let next_task = self.tasks.back().unwrap();
            let next_ctx = next_task.ctx.unwrap();
            unsafe {
                self.tasks[self.tasks.len()-2].ctx.unwrap().pivot(&next_ctx);
            }
        }

        fn add(&mut self, t: Task) {
            self.tasks.push_front(t);
        }

        fn remove(&mut self) {
            assert!(is_x86_feature_detected!("avx")); // for vstmxcsr
            if self.tasks.back().unwrap().stack.len() == 0 {
                panic!("can't remove main task");
            }
            if self.tasks.len() == 1 { // TODO: error, not panic
                panic!("can't remove only task");
            }
            assert_ne!(self.tasks.len(), 0);

            let mut active = self.tasks.pop_back().unwrap();

            {
                let t = self.tasks.pop_front().unwrap();
                self.tasks.push_back(t);
            }

            unsafe {
                active.ctx.unwrap().pivot(
                    &self.tasks.back().unwrap().ctx.unwrap()
                );
            }
        }

/*
 *        pub fn add<T: Send>(
 *            &mut self,
 *            f: extern "sysv64" fn(&mut T) -> !,
 *            arg: T,
 *        ) {
 *            let mut t = Task {
 *                stack: Vec::with_capacity(1<<18),
 *                ctx: Some(Context {
 *                    rip: f as *const u8,
 *                    rsp: ptr::null_mut(),
 *                    rbp: ptr::null_mut(),
 *                }),
 *            };
 *
 *            unsafe { t.stack.set_len(1<<18); }
 *
 *            if let Some(ref mut ctx) = t.ctx {
 *                ctx.rsp = t.stack.last_mut().unwrap() as *mut u8;
 *
 *                unsafe { push_raw(&mut ctx.rsp, arg); }
 *                let arg_ref = ctx.rsp;
 *
 *                ctx.rsp = align_mut_ptr_down(ctx.rsp, 16);
 *                ctx.rsp -= 8;
 *                unsafe { ptr::write_bytes(ctx.rsp, 0, 8); }
 *            } else { panic!(); }
 *
 *            self.tasks.insert(t);
 *        }
 */
    }

            /*
             *next_task.with(|rc_nt| {
             *    let (nt_rip, nt_rsp, nt_rbp) = {
             *        let mut nt = nt_rc.borrow_mut();
             *        (
             *            &mut nt.rip as *mut *const u8,
             *            &mut nt.rsp as *mut *mut u8,
             *            &mut nt.rbp as *mut *mut u8,
             *        )
             *    };
             *});
             */

    thread_local! {
        static active_pool: RefCell<Option<Box<dyn Pool>>> = RefCell::new(None);
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

        active_pool.with(|ap| {
            if let Some(ref mut ap) = *ap.borrow_mut() {
                ap.add(t);
                ap.next();
            } else { panic!(); }
        });
    }

    pub fn next() {
        active_pool.with(|ap| {
            if let Some(ref mut ap) = *ap.borrow_mut() {
                ap.next();
            } else { panic!(); }
        });
    }
}
