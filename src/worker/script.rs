extern crate llvm_sys as llvm;

use std::{
    collections::HashMap, fmt::Display, fs::OpenOptions, io::Write,
    process::Command, thread, time,
};

use log::{debug, info};
use rand::{distributions::Alphanumeric, thread_rng, Rng};
use rand_distr::Exp;

use llvm::core::*;
use llvm::execution_engine::*;
use llvm::target::*;
use llvm_sys::prelude::*;
use std::ffi::{c_void, CStr};
use std::mem;

use crate::{Worker, WorkerError};

use crate::script::ast::{Arg, Dist, Instruction, Node};

#[derive(Debug, Clone)]
pub struct ScriptWorker {
    node: Node,
    jit: extern "C" fn() -> u64,
    ee: LLVMExecutionEngineRef,
    context: LLVMContextRef,
}

/// # Safety
///
/// Log the input at debug level.
#[no_mangle]
pub unsafe extern "C" fn debug(text: *const i8) -> u64 {
    let text = unsafe { CStr::from_ptr(text) };
    debug!("{}", text.to_str().unwrap());
    0
}

/// # Safety
///
/// Open a file with create and write permissions and write to it.
#[no_mangle]
pub unsafe extern "C" fn open(path: *const i8) -> u64 {
    let path = unsafe { CStr::from_ptr(path).to_str().unwrap() };
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path)
        .unwrap();
    file.write_all(b"Test").unwrap();
    0
}

/// # Safety
///
/// Spawn a process with a random argument.
#[no_mangle]
pub unsafe extern "C" fn task(name: *const i8) -> u64 {
    let name = unsafe { CStr::from_ptr(name) };
    let uniq_arg: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();
    let _res = Command::new(name.to_str().unwrap())
        .arg(uniq_arg)
        .output()
        .unwrap();
    0
}

pub struct RuntimeFunc {
    name: &'static str,
    // func: extern "C" fn(*const i8) -> u64,
}

pub static RUNTIME: [RuntimeFunc; 3] = [
    RuntimeFunc {
        name: "task",
        // func: task
    },
    RuntimeFunc {
        name: "debug",
        // func: debug,
    },
    RuntimeFunc {
        name: "open",
        // func: open,
    },
];

impl ScriptWorker {
    pub fn new(node: Node) -> Self {
        let mut module_runtime: HashMap<String, (LLVMValueRef, LLVMTypeRef)> =
            HashMap::new();
        let mut module_state: HashMap<String, LLVMValueRef> = HashMap::new();

        unsafe {
            // Set up a context, module and builder in that context.
            let context = LLVMContextCreate();
            let module = LLVMModuleCreateWithNameInContext(
                c"main".as_ptr() as *const _,
                context,
            );
            let builder = LLVMCreateBuilderInContext(context);

            // Robust code should check that these calls complete successfully.
            // Each of calls is necessary to setup an execution engine which
            // compiles to native code.
            LLVMLinkInMCJIT();
            LLVM_InitializeNativeTarget();
            LLVM_InitializeNativeAsmPrinter();

            // Build an execution engine.
            let ee = {
                let mut ee = mem::MaybeUninit::uninit();
                let mut err = mem::zeroed();
                // This moves ownership of the module into the execution engine.
                if LLVMCreateExecutionEngineForModule(
                    ee.as_mut_ptr(),
                    module,
                    &mut err,
                ) != 0
                {
                    // In case of error, we must avoid using the uninitialized ExecutionEngineRef.
                    assert!(!err.is_null());
                    panic!(
                        "Failed to create execution engine: {:?}",
                        CStr::from_ptr(err)
                    );
                }
                ee.assume_init()
            };

            // get a type for main function
            let i64t = LLVMInt64TypeInContext(context);
            let td = LLVMGetExecutionEngineTargetData(ee);
            let iptr = LLVMIntPtrTypeInContext(context, td);

            let mut argts = [];
            let function_type = LLVMFunctionType(
                i64t,
                argts.as_mut_ptr(),
                argts.len() as u32,
                0,
            );

            for f in &RUNTIME {
                if module_runtime.contains_key(f.name) {
                    break;
                };

                let mut task_argts = [iptr];
                let function_type = LLVMFunctionType(
                    i64t,
                    task_argts.as_mut_ptr(),
                    task_argts.len() as u32,
                    0,
                );
                let func = LLVMAddFunction(
                    module,
                    format!("{}\0", f.name).into_bytes().as_ptr() as *const _,
                    function_type,
                );
                debug!("Insert {} into runtime", f.name);
                module_runtime
                    .insert(f.name.to_string(), (func, function_type));
            }

            // add it to our module
            let function = LLVMAddFunction(
                module,
                c"main".as_ptr() as *const _,
                function_type,
            );

            // Create a basic block in the function and set our builder to generate
            // code in it.
            let bb = LLVMAppendBasicBlockInContext(
                context,
                function,
                c"entry".as_ptr() as *const _,
            );
            LLVMPositionBuilderAtEnd(builder, bb);

            let stub_ptr = LLVMBuildGlobalString(
                builder,
                c"stub".as_ptr() as *const _,
                c"name".as_ptr() as *const _,
            );
            module_state.insert(String::from("stub"), stub_ptr);

            let Node::Work {
                name: _,
                args: _,
                instructions,
                dist: _,
            } = node.clone();
            for instr in instructions {
                match instr {
                    Instruction::Task { name, args } => {
                        let task_name = args[0].clone();
                        let mut arg_ptr;

                        match task_name {
                            Arg::Const { text } => {
                                arg_ptr = LLVMBuildGlobalString(
                                    builder,
                                    format!("{text}\0").as_ptr() as *const _,
                                    c"const".as_ptr() as *const _,
                                );
                            }
                            Arg::Var { name } => {
                                arg_ptr = *module_state.get(&name).unwrap();
                            }
                        }

                        let (func, func_type) =
                            module_runtime.get(&name).unwrap();
                        LLVMBuildCall2(
                            builder,
                            *func_type,
                            *func,
                            &mut arg_ptr,
                            1,
                            c"task".as_ptr() as *const _,
                        );
                    }
                    unknown => panic!("Unknown instruction: {unknown:?}"),
                }
            }

            // Emit a `ret i64` into the function to return the computed sum.
            let ret = LLVMConstInt(i64t, 0, 0);
            LLVMBuildRet(builder, ret);
            // done building
            LLVMDisposeBuilder(builder);
            // Dump the module as IR to stdout.
            LLVMDumpModule(module);

            let Node::Work {
                name: _,
                args: _,
                instructions,
                dist: _,
            } = node.clone();
            for instr in instructions {
                match instr {
                    Instruction::Task { name, args: _ } => {
                        let func = LLVMGetNamedFunction(
                            module,
                            format!("{name}\0").into_bytes().as_ptr()
                                as *const _,
                        );

                        let task = match name.as_str() {
                            "task" => task,
                            "debug" => debug,
                            "open" => open,
                            unknown => {
                                panic!("Unknown instruction: {unknown:?}")
                            }
                        };

                        debug!("Add mapping to {:?}", name);
                        LLVMAddGlobalMapping(ee, func, task as *mut c_void);
                    }
                    unknown => panic!("Unknown instruction: {unknown:?}"),
                }
            }

            let addr = LLVMGetFunctionAddress(ee, c"main".as_ptr() as *const _);
            let jit: extern "C" fn() -> u64 = mem::transmute(addr);
            ScriptWorker {
                node,
                jit,
                ee,
                context,
            }
        }
    }
}

impl Worker for ScriptWorker {
    fn run_payload(&self) -> Result<(), WorkerError> {
        let Node::Work {
            name: _,
            args: _,
            instructions: _,
            dist,
        } = self.node.clone();

        match dist {
            Some(d) => {
                let Dist::Exp { rate } = d;

                loop {
                    let worker = self.clone();
                    thread::spawn(move || {
                        (worker.jit)();
                    });

                    let interval: f64 =
                        thread_rng().sample(Exp::new(rate).unwrap());
                    info!(
                        "Interval {}, rounded {}",
                        interval,
                        (interval * 1000.0).round() as u64,
                    );
                    thread::sleep(time::Duration::from_millis(
                        (interval * 1000.0).round() as u64,
                    ));
                }
            }
            None => (self.jit)(),
        };

        unsafe {
            LLVMDisposeExecutionEngine(self.ee);
            LLVMContextDispose(self.context);
        }

        Ok(())
    }
}

impl Display for ScriptWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self.node)
    }
}
