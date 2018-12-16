#![feature(asm, naked_functions)]

use std::is_x86_feature_detected;

// #[cfg(all(unix, target_arch = "x86_64"))]
// pub use self::x86_64_unix::???;

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
        pub unsafe fn call<F>(f: F) -> Context
        where
            F: Send + FnOnce(Context),
        {
            // I think we need this for vstmxcsr.
            assert!(
                is_x86_feature_detected!("avx")
            );

            // 1. allocate stack
            // 2. save state

            Context {
                rbp: 
            }
        }

        /// self: context to activate
        /// return value: activator's context
        pub unsafe extern "C" fn activate(self) -> Context {
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
                    fstcw $3
                    vstmxcsr $4

                    mov r8, $5
                    mov r9, $6
                    mov rcx, $7

                    mov r12, rbp
                    mov r13, rsp
                    lea r14, [rip+back]

                    mov rbp, r8
                    mov rsp, r9
                    jmp rcx
                back:
                    mov $0, r12
                    mov $1, r13
                    mov $2, r14

                    fldcw $3
                    vldmxcsr $4
                "
            :
                "=r"(prev_rbp),
                "=r"(prev_rsp),
                "=r"(prev_rip),
                "=&*m"(&mut our_fcw)
                "=&*m"(&mut our_mxcsr)
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
