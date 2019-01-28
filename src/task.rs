pub use self::platform::{
    next,
    start,
    Task,
};

pub fn wait_result<X, E1, E2, F, G>(mut f: F, mut g: G) -> Result<X, E2>
where
    F: FnMut() -> Result<X, E1>,
    G: FnMut(E1) -> Option<E2>,
{
    loop {
        match f() {
            Ok(x) => return Ok(x),
            Err(e) => match g(e) {
                Some(e) => return Err(e),
                None => (),
            },
        }
        next();
    }
}

#[cfg(all(unix, target_arch = "x86_64"))]
mod platform {
    use crate::util::{
        align_mut_ptr_down,
        push_raw,
        steal_mut,
    };
    use std::boxed::FnBox;
    use std::cell::RefCell;
    use std::collections::VecDeque;
    use std::fmt;
    use std::ptr;

    #[no_mangle]
    extern "sysv64" fn go(f: &mut Option<Box<dyn FnBox()>>) {
        let b: Box<_> = f.take().unwrap();
        b()
    }

    extern "sysv64" {
        fn start_inner(
            f: *mut u8, // rdi
            rsp: *mut u8, // rsi
            save_ctx: *mut u8, // rdx
        ) -> bool;

        fn pivot_inner(
            rip: *const u8, // rdi
            rsp: *mut u8, // rsi
            rbp: *mut u8, // rdx
            done: bool, // rcx
            save_ctx: *mut u8, // r8
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

            mov r12, rsi # new rsp

            lea rax, [rip + pivot_inner_back]
            mov [rdx], rax
            mov [rdx + 8], rsp
            mov [rdx + 16], rbp

            mov rsp, r12
            # rbp doesn't matter
            push 0 # align stack
            emms
            call go
            pop rax # don't care
            jmp done

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
            mov r14, rcx # done

            lea rax, [rip + pivot_inner_back]
            mov [r8], rax
            mov [r8 + 8], rsp
            mov [r8 + 16], rbp

            mov rsp, r12
            mov rbp, r13
            jmp r11

        pivot_inner_back:
            mov rax, r14 # done

            fldcw [rsp]
            pop rdx # don't care
            vldmxcsr [rsp]
            pop rdx # don't care
            pop r15
            pop r14
            pop r13
            pop r12
            pop rbx
            pop rbp

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
            f: Option<(*mut u8, *mut u8)>,
            done: bool,
        ) -> bool {
            assert!(is_x86_feature_detected!("avx")); // for vstmxcsr

            if let Some((_, f)) = f {
                start_inner(
                    f,
                    next.rsp,
                    self as *mut Context as *mut u8, // I guess
                )
            } else {
                pivot_inner(
                    next.rip,
                    next.rsp,
                    next.rbp,
                    done,
                    self as *mut Context as *mut u8, // I guess
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

    pub fn start<F: FnOnce() + Send + 'static>(
        f: F,
    ) {
        assert!(is_x86_feature_detected!("avx")); // for vstmxcsr

        let mut stack = Vec::with_capacity(1<<18);

        unsafe { stack.set_len(1<<18); }
        let mut rsp = stack.last_mut().unwrap() as *mut u8;

        let f = Some(Box::new(f) as Box<dyn FnBox()>);
        unsafe {
            push_raw(&mut rsp, f);
        }
        let f = rsp;

        rsp = align_mut_ptr_down(rsp, 16);
        rsp = unsafe { rsp.offset(-8) };
        unsafe { ptr::write_bytes(rsp, 0, 8); } // "return address"

        let t = Task {
            stack,
            ctx: Some(Context {
                rip: ptr::null(),
                rsp,
                rbp: ptr::null_mut(),
            }),
        };

        // back = active, front = next

        TASKS.with(|tt| {
            let mut tt = tt.borrow_mut();
            tt.push_front(t);
        });
        pivot(Some((ptr::null_mut(), f)), false);
    }

    fn pivot(f: Option<(*mut u8, *mut u8)>, done: bool) {
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
                active_ctx.pivot(&next_ctx, f, done)
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
    extern "sysv64" fn done() {
        pivot(None, true)
    }
}
