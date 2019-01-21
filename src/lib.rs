#![feature(global_asm)]

use std::mem::{align_of, size_of};
use std::ptr;

/// *extremely* unsafe
pub(crate) unsafe fn steal_mut<'a, T>(x: &mut T) -> &'a mut T {
    &mut *(x as *mut T)
}

pub(crate) fn align_mut_ptr_down<T>(p: *mut T, alignment: usize) -> *mut T {
    ((p as usize) & !((1<<alignment) - 1)) as *mut T
}

// TODO: Use preferred alignment?
/// p need not be aligned.
pub(crate) unsafe fn push_raw<T>(p: &mut *mut u8, val: T) {
    *p = align_mut_ptr_down(*p, align_of::<T>());
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
        steal_mut,
    };
    use std::any::Any;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::fmt;
    use std::ptr;

    // `done` must be r8 for both functions.
    extern "sysv64" {
        fn start_inner(
            rip: *const u8, // rdi
            rsp: *mut u8, // rsi
            save_ctx: *mut u8, // rdx
            done: bool, // rcx
            arg: *mut u8, // r8
            arg_box: *mut u8, // r9
        ) -> bool;

        fn pivot_inner(
            rip: *const u8, // rdi
            rsp: *mut u8, // rsi
            rbp: *mut u8, // rdx
            save_ctx: *mut u8, // rcx
            done: bool, // r8
        ) -> bool;
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

            mov r11, rdi # new rip
            mov r12, rsi # new rsp

            lea rax, [rip + pivot_inner_back]
            mov [rdx], rax
            mov [rdx + 8], rsp
            mov [rdx + 16], rbp

            mov rsp, r12
            # rbp doesn't matter
            mov rdi, r8 # arg
            push r9 # conveniently needed to align stack
            emms
            call r11
            pop rdi # arg_box
            jmp done
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

            mov r11, rdi # new rip
            mov r12, rsi # new rsp
            mov r13, rdx # new rbp

            lea rax, [rip + pivot_inner_back]
            mov [rcx], rax
            mov [rcx + 8], rsp
            mov [rcx + 16], rbp

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

            mov rax, r8 # done
            ret  # TODO: far?
    "#);

    #[derive(Clone, Copy, Debug)]
    #[repr(C)]
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

        // TODO: bit weird to go from start() to pivot() to start_inner()
        pub(crate) unsafe fn pivot(
            &mut self,
            next: &Context,
            arg: Option<(*mut u8, *mut u8)>,
            done: bool,
        ) -> bool {
            assert!(is_x86_feature_detected!("avx")); // for vstmxcsr

            if let Some((arg, arg_box)) = arg {
                start_inner(
                    next.rip,
                    next.rsp,
                    self as *mut Context as *mut u8, // I guess
                    done,
                    arg,
                    arg_box,
                )
            } else {
                pivot_inner(
                    next.rip,
                    next.rsp,
                    next.rbp,
                    self as *mut Context as *mut u8, // I guess
                    done,
                )
            }
        }
    }

    pub struct Task {
        stack: Vec<u8>,
        ctx: Option<Context>,
    }

    impl fmt::Debug for Task {
        fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
            write!(f, "Task({}, {:?})", self.stack.len(), self.ctx)
        }
    }

    thread_local! {
        // back = active, front = next
        static TASKS: RefCell<VecDeque<Task>> = RefCell::new(
            vec![Task { stack: vec![], ctx: None }].into()
        );
    }

    pub fn start<T: Send + 'static>(
        f: extern "sysv64" fn(&mut T),
        arg: T,
    ) {
        assert!(is_x86_feature_detected!("avx")); // for vstmxcsr

        let mut stack = Vec::with_capacity(1<<18);

        unsafe { stack.set_len(1<<18); }
        let mut rsp = stack.last_mut().unwrap() as *mut u8;

        let mut arg_box = Box::new(arg) as Box<dyn Any>;
        let arg = arg_box.downcast_mut::<T>().unwrap() as *mut T; // not ideal
        unsafe {
            push_raw(&mut rsp, arg_box);
        }
        let arg_box = rsp;

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

        TASKS.with(|tt| {
            let mut tt = tt.borrow_mut();
            tt.push_front(t);
        });
        pivot(Some((arg as *mut u8, arg_box)), false);
    }

    fn pivot(arg: Option<(*mut u8, *mut u8)>, done: bool) {
        // back = active, front = next

        let ctxs = TASKS.with(|tt| {
            let mut tt = tt.borrow_mut();

            if done && tt.back().unwrap().stack.len() == 0 {
                panic!("main task is not allowed to finish");
            }

            assert_ne!(tt.len(), 0);
            if tt.len() == 1 {
                return None;
            }

            let t = tt.pop_front().unwrap();
            tt.push_back(t);

            let next_ctx = {
                let next_task = tt.back_mut().unwrap();
                next_task.ctx.take().unwrap()
            };

            let active_i = tt.len() - 2;
            tt[active_i].ctx = Some(Context::null());
            let active_ctx = unsafe {
                steal_mut(tt[active_i].ctx.as_mut().unwrap())
            };

            // We stole active_ctx, so it *must not* survive past the end of
            // pivot(), and we *must not* modify TASKS until then.

            Some((active_ctx, next_ctx))
        });

        let activator_done = if let Some((active_ctx, next_ctx)) = ctxs {
            unsafe {
                active_ctx.pivot(&next_ctx, arg, done)
            }
        } else {
            false
        };

        if activator_done {
            TASKS.with(|tt| {
                let mut tt = tt.borrow_mut();

                assert!(tt.len() > 1);

                let activator_i = tt.len() - 2;
                tt.remove(activator_i);
            });
        }
    }

    pub fn next() {
        pivot(None, false)
    }

    #[no_mangle]
    extern "sysv64" fn done(arg_box: &mut Box<dyn Any>) {
        *arg_box = Box::new(());

        pivot(None, true)
    }
}
