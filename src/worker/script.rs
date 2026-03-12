extern crate llvm_sys as llvm;

use std::{
    collections::HashMap,
    fmt::Display,
    fs::OpenOptions,
    io::Write,
    io::prelude::*,
    net::{Shutdown, TcpStream},
    process::Command,
    thread, time,
};

use std::sync::LazyLock;

use log::{Level, debug, log_enabled, trace};
use rand::{Rng, distributions::Alphanumeric, thread_rng};
use rand_distr::Exp;

use llvm::core::*;
use llvm::execution_engine::*;
use llvm::target::*;
use llvm_sys::LLVMType;
use llvm_sys::prelude::*;
use std::ffi::{CStr, CString, c_void};
use std::mem;

use crate::{Worker, WorkerError};

use crate::script::ast::{Arg, Dist, Instruction, Node};

enum RuntimeType {
    Int,
    Pointer,
}

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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn debug(text: *const i8) -> u64 {
    let text = unsafe { CStr::from_ptr(text) };
    debug!("{}", text.to_str().unwrap());
    0
}

/// # Safety
///
/// Open a file with create and write permissions and write to it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn open(path: *const i8) -> u64 {
    //let path = unsafe { CString::from_raw(path as *mut i8) };
    let path = unsafe { CStr::from_ptr(path) };
    debug!("Open path {:?}", path);
    let mut file = OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(path.to_str().unwrap())
        .unwrap();
    file.write_all(b"Test").unwrap();
    0
}

/// # Safety
///
/// Open a file with create and write permissions and write to it.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ping(addr: *const i8) -> u64 {
    let addr = unsafe { CStr::from_ptr(addr).to_str().unwrap() };
    debug!("Ping {:?}", addr);
    let mut stream =
        TcpStream::connect(addr).expect("Couldn't connect to the server...");

    stream.write_all(b"Hello\n").unwrap();
    let mut buf = vec![];
    stream.read_exact(&mut buf).unwrap();

    stream
        .shutdown(Shutdown::Both)
        .expect("shutdown call failed");
    0
}

/// # Safety
///
/// Spawn a process with a random argument.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn task(name: *const i8, random: bool) -> u64 {
    let name = unsafe { CStr::from_ptr(name) };
    debug!("Task {:?} {:?}", name, random);
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

/// # Safety
///
/// Return a randomly generated path.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn random_path(base: *const i8) -> *const i8 {
    let base = unsafe { CStr::from_ptr(base).to_string_lossy().into_owned() };

    let uniq: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();

    CString::new(format!("{base}/{uniq}")).unwrap().into_raw()
}

pub struct RuntimeFunc {
    name: &'static str,
    func: usize,
    param_count: u32,
    param_types: &'static [RuntimeType],
    return_type: RuntimeType,
}

/// Functions, available in a script at runtime
pub static RUNTIME: LazyLock<[RuntimeFunc; 5]> = LazyLock::new(|| {
    [
        // workload support
        RuntimeFunc {
            name: "task",
            func: task as *const () as usize,
            param_count: 1,
            param_types: &[RuntimeType::Pointer],
            return_type: RuntimeType::Int,
        },
        RuntimeFunc {
            name: "debug",
            func: debug as *const () as usize,
            param_count: 1,
            param_types: &[RuntimeType::Pointer],
            return_type: RuntimeType::Int,
        },
        RuntimeFunc {
            name: "open",
            func: open as *const () as usize,
            param_count: 1,
            param_types: &[RuntimeType::Pointer],
            return_type: RuntimeType::Int,
        },
        RuntimeFunc {
            name: "ping",
            func: ping as *const () as usize,
            param_count: 1,
            param_types: &[RuntimeType::Pointer],
            return_type: RuntimeType::Int,
        },
        // dynamic values
        RuntimeFunc {
            name: "random_path",
            func: random_path as *const () as usize,
            param_count: 1,
            param_types: &[RuntimeType::Pointer],
            return_type: RuntimeType::Pointer,
        },
    ]
});

impl ScriptWorker {
    fn jit_instruction(
        name: String,
        arg: Arg,
        builder: LLVMBuilderRef,
        ee: LLVMExecutionEngineRef,
        module: LLVMModuleRef,
        module_state: &HashMap<String, LLVMValueRef>,
        module_runtime: &HashMap<String, (LLVMValueRef, LLVMTypeRef)>,
    ) {
        let mut arg_ptr = Self::get_arg_value(
            arg,
            builder,
            ee,
            module,
            module_state,
            module_runtime,
        );

        let (func, func_type) = module_runtime.get(&name).unwrap();

        unsafe {
            LLVMBuildCall2(
                builder,
                *func_type,
                *func,
                &mut arg_ptr,
                1,
                name.as_str().as_ptr() as *const _,
            );
        }
    }

    fn get_arg_value(
        arg: Arg,
        builder: LLVMBuilderRef,
        ee: LLVMExecutionEngineRef,
        module: LLVMModuleRef,
        module_state: &HashMap<String, LLVMValueRef>,
        module_runtime: &HashMap<String, (LLVMValueRef, LLVMTypeRef)>,
    ) -> LLVMValueRef {
        match arg {
            Arg::Const { text } => unsafe {
                LLVMBuildGlobalString(
                    builder,
                    format!("{text}\0").as_ptr() as *const _,
                    c"const".as_ptr() as *const _,
                )
            },
            Arg::Var { name } => *module_state.get(&name).expect("No variable"),
            Arg::Dynamic { name, args } => {
                let (func, func_type) = module_runtime
                    .get(&name)
                    .expect("No dynamic variable in the runtime");

                let runtime_func = &RUNTIME
                    .iter()
                    .find(|f| f.name == name)
                    .expect("No dynamic variable in the static runtime");

                let text = match &args[0] {
                    Arg::Const { text } => text,
                    unknown => {
                        panic!("Unknown dynamic variable argument: {unknown:?}")
                    }
                };

                unsafe {
                    trace!("Add mapping to {:?}", name);
                    let module_func = LLVMGetNamedFunction(
                        module,
                        format!("{name}\0").into_bytes().as_ptr() as *const _,
                    );

                    LLVMAddGlobalMapping(
                        ee,
                        module_func,
                        runtime_func.func as *mut c_void,
                    );

                    let mut helper_ptr = LLVMBuildGlobalString(
                        builder,
                        format!("{text}\0").as_ptr() as *const _,
                        c"const".as_ptr() as *const _,
                    );

                    LLVMBuildCall2(
                        builder,
                        *func_type,
                        *func,
                        &mut helper_ptr,
                        1,
                        c"{name}".as_ptr() as *const _,
                    )
                }
            }
        }
    }

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

            let td = LLVMGetExecutionEngineTargetData(ee);

            // get a type for main function
            let i64t = LLVMInt64TypeInContext(context);
            let iptr = LLVMIntPtrTypeInContext(context, td);

            // Insert runtime functions into the module
            for f in &*RUNTIME {
                if module_runtime.contains_key(f.name) {
                    break;
                };

                let mut function_args = f
                    .param_types
                    .iter()
                    .map(|t| match t {
                        RuntimeType::Pointer => iptr,
                        RuntimeType::Int => i64t,
                    })
                    .collect::<Vec<*mut LLVMType>>();

                let function_type = LLVMFunctionType(
                    match f.return_type {
                        RuntimeType::Int => i64t,
                        RuntimeType::Pointer => iptr,
                    },
                    function_args.as_mut_ptr(),
                    f.param_count,
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

            let mut argts = [];
            let function_type = LLVMFunctionType(
                i64t,
                argts.as_mut_ptr(),
                argts.len() as u32,
                0,
            );

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

            // Insert stub variable.
            // XXX: Move to the runtime data
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
            } = node.clone()
            else {
                unreachable!()
            };

            // Iterate proviled instructions and convert to JIT
            for instr in instructions {
                match instr {
                    Instruction::Task { name, args: _ } => {
                        Self::jit_instruction(
                            String::from("task"),
                            name,
                            builder,
                            ee,
                            module,
                            &module_state,
                            &module_runtime,
                        );
                    }

                    Instruction::Open { path } => {
                        Self::jit_instruction(
                            String::from("open"),
                            path,
                            builder,
                            ee,
                            module,
                            &module_state,
                            &module_runtime,
                        );
                    }

                    Instruction::Ping { server } => {
                        Self::jit_instruction(
                            String::from("ping"),
                            server,
                            builder,
                            ee,
                            module,
                            &module_state,
                            &module_runtime,
                        );
                    }

                    Instruction::Debug { text } => {
                        Self::jit_instruction(
                            String::from("debug"),
                            text,
                            builder,
                            ee,
                            module,
                            &module_state,
                            &module_runtime,
                        );
                    }
                }
            }

            // Emit a `ret i64` into the function to return the computed sum.
            let ret = LLVMConstInt(i64t, 0, 0);
            LLVMBuildRet(builder, ret);
            // done building
            LLVMDisposeBuilder(builder);

            if log_enabled!(Level::Debug) {
                // Dump the module as IR to stdout.
                LLVMDumpModule(module);
            }

            let Node::Work {
                name: _,
                args: _,
                instructions,
                dist: _,
            } = node.clone()
            else {
                unreachable!()
            };

            // Populate the global mapping with observed runtime functions
            for instr in instructions {
                let name = match instr {
                    Instruction::Task { .. } => "task",
                    Instruction::Open { .. } => "open",
                    Instruction::Debug { .. } => "debug",
                    Instruction::Ping { .. } => "ping",
                };

                trace!("Add mapping to {:?}", name);

                let module_func = LLVMGetNamedFunction(
                    module,
                    format!("{name}\0").into_bytes().as_ptr() as *const _,
                );

                let runtime_func = &RUNTIME
                    .iter()
                    .find(|f| f.name == name)
                    .expect("No runtime function with the name");

                LLVMAddGlobalMapping(
                    ee,
                    module_func,
                    runtime_func.func as *mut c_void,
                );
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
        } = self.node.clone()
        else {
            unreachable!()
        };

        match dist {
            Some(d) => {
                debug!("Distribution {:?}", d);
                let Dist::Exp { rate } = d else { todo!() };

                loop {
                    let worker = self.clone();
                    thread::spawn(move || {
                        (worker.jit)();
                    });

                    let interval: f64 =
                        thread_rng().sample(Exp::new(rate).unwrap());
                    debug!(
                        "Interval {}, rounded {}",
                        interval,
                        (interval * 1000.0).round() as u64,
                    );
                    thread::sleep(time::Duration::from_millis(
                        (interval * 1000.0).round() as u64,
                    ));
                }
            }
            None => {
                debug!("Single unit");
                (self.jit)()
            }
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
