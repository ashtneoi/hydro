#![feature(asm)]

use std::is_x86_feature_detected;

// #[cfg(all(unix, target_arch = "x86_64"))]
pub use self::x86_64_unix::Context;

pub extern "C" fn thing(rbp: *mut u8, c: Context) {
    println!("about to pivot stack");

    // pivot stack

    let prev_rbp: *mut u8;
    let prev_rsp: *mut u8;
    let prev_rip: *mut u8;

    unsafe {
        asm!(
            "
                mov r8, $3
                mov r9, $4
                mov rcx, $5

                mov rax, $6
                and rax, 0xFFFFFFFFFFFFFFF0

                mov rbp, rax
                lea rsp, [rax - 128]
                sub rsp, 128
                sub rsp, 128

                mov $0, r8
                mov $1, r9
                mov $2, rcx
            "
        :
            "=r"(prev_rbp),
            "=r"(prev_rsp),
            "=r"(prev_rip)
        :
            "r"(c.rbp),
            "r"(c.rsp),
            "r"(c.rip),
            "r"(rbp)
        :
            "r8", "r9", "rcx",
            "rax",
            "memory"
        :
            "intel", "volatile"
        );
    }

    // parameters are gone!

    let mut c = Context {
        rbp: prev_rbp,
        rsp: prev_rsp,
        rip: prev_rip,
    };

    println!("thing pivoted stack!");

    unsafe {
        loop {
            println!("right before activate()");
            c = c.activate();
        }
    }

    println!("thing was reactivated!");

    unreachable!(); // lie
}

// TODO: do we also need to specify pointer size = 64?
#[cfg(all(unix, target_arch = "x86_64"))]
mod x86_64_unix {
    /// a jump destination
    pub struct Context {
        pub(crate) rbp: *mut u8,
        pub(crate) rsp: *mut u8,
        pub(crate) rip: *mut u8,
    }

    impl Context {
        pub unsafe extern "C" fn call(f: extern "C" fn(*mut u8, Context))
            -> Context
        {
            // I think we need this for vstmxcsr.
            assert!(
                is_x86_feature_detected!("avx")
            );

            // 1. allocate stack
            // 2. save state

            let mut stack: Vec<u8> = vec![0; 1<<14];

            let next_rbp = (&mut stack[(1<<14)-16]) as *mut u8;

            let our_rbp: *mut u8;
            let our_rsp: *mut u8;
            let our_rip: *mut u8;

            let mut our_fcw: u16 = 0;
            let mut our_mxcsr: u32 = 0;

            let prev_rbp: *mut u8;
            let prev_rsp: *mut u8;
            let prev_rip: *mut u8;

            println!("main task about to call thing");

            asm!(
                "
                    mov $0, rbp
                    mov $1, rsp
                    lea $2, [rip+back_5ebe61aa363e6893]

                    vstmxcsr $4
                "
            :
                "=r"(our_rbp),
                "=r"(our_rsp),
                "=r"(our_rip),
                "=*m"(&mut our_fcw),
                "=*m"(&mut our_mxcsr)
            :
            :
            :
                "intel"
            );

            println!("here we go");

            f(
                next_rbp,
                Context {
                    rbp: our_rbp,
                    rsp: our_rsp,
                    rip: our_rip,
                }
            );

            asm!(
                "
                back_5ebe61aa363e6893:
                    mov $0, r12
                    mov $1, r13
                    mov $2, r14

                    vldmxcsr $4
                "
            :
                "=r"(prev_rbp),
                "=r"(prev_rsp),
                "=r"(prev_rip)
            :
                "*m"(&our_fcw),
                "*m"(&our_mxcsr)
            :
                "r8", "r9", "rcx", // prev
                "r12", "r13", "r14", // ours
                "rbx", "r15", // callee-save
                "memory"
            :
                "intel", "volatile"
            );

            println!("main task was activated");

            Context {
                rbp: prev_rbp,
                rsp: prev_rsp,
                rip: prev_rip,
            }
        }

        /// self: context to activate
        /// return value: activator's context
        pub unsafe extern "C" fn activate(self) -> Context {
            println!("just inside activate()");

            let mut our_fcw: u16 = 0;
            let mut our_mxcsr: u32 = 0;

            let prev_rbp: *mut u8;
            let prev_rsp: *mut u8;
            let prev_rip: *mut u8;

            // 1. save state
            // 2. load activatee state
            // 3. jump to activatee
            // (jump target)
            // 4. save activator state
            asm!(
                "
                    mov r8, $5
                    mov r9, $6
                    mov rcx, $7

                    vstmxcsr $4

                    mov r12, rbp
                    mov r13, rsp
                    lea r14, [rip+back_b3c037d6b3912998]

                    mov rbp, r8
                    mov rsp, r9
                    jmp rcx
                back_b3c037d6b3912998:
                    mov $0, r12
                    mov $1, r13
                    mov $2, r14

                    vldmxcsr $4
                "
            :
                "=r"(prev_rbp),
                "=r"(prev_rsp),
                "=r"(prev_rip),
                "=*m"(&mut our_fcw),
                "=*m"(&mut our_mxcsr)
            :
                "r"(self.rbp),
                "r"(self.rsp),
                "r"(self.rip)
            :
                "r8", "r9", "rcx", // prev
                "r12", "r13", "r14", // ours
                "rbx", "r15", // callee-save
                "memory"
            :
                "intel", "volatile"
            );

            println!("prev_rsp = {:?}", prev_rsp);

            Context {
                rbp: prev_rbp,
                rsp: prev_rsp,
                rip: prev_rip,
            }
        }
    }

    /// a thread of execution
    pub struct Task {
        stack: Box<[u8]>,
        context: Context,
    }

    impl Task {
    }

    /// a cooperative scheduling group
    pub struct Group(Vec<Task>);

    impl Group {
        pub fn new() -> Self {
            Group(vec![])
        }

        //pub fn spawn<F>(&mut self, f: F)
        //where
            //F: Send + FnOnce(Context),
        //{
        //}
    }
}
