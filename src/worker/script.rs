extern crate llvm_sys as llvm;

use std::{
    cell::RefCell,
    collections::HashMap,
    fmt::Display,
    fs::OpenOptions,
    io::Write,
    io::prelude::*,
    net::{Shutdown, TcpListener, TcpStream},
    os::fd::{AsRawFd, RawFd},
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread, time,
};

use std::sync::LazyLock;

use log::{Level, debug, log_enabled, trace};
use rand::{Rng, distributions::Alphanumeric, thread_rng};
use rand_distr::{Exp, Zipf};

use llvm::core::*;
use llvm::execution_engine::*;
use llvm::target::*;
use llvm_sys::LLVMType;
use llvm_sys::prelude::*;
use std::ffi::{CStr, CString, c_void};
use std::mem;

use crate::{Worker, WorkerError};

use crate::script::ast::{Arg, ConstType, Dist, Instruction, Node};

#[derive(Debug, Clone)]
enum RuntimeType {
    Int,
    Pointer,
    Float,
}

#[derive(Debug, Clone)]
pub struct ScriptWorker {
    node: Node,
    jit: extern "C" fn() -> u64,
    ee: LLVMExecutionEngineRef,
    context: LLVMContextRef,
}

#[derive(Debug, Clone)]
struct BuildContext<'a> {
    ee: LLVMExecutionEngineRef,
    builder: LLVMBuilderRef,
    module: LLVMModuleRef,
    context: LLVMContextRef,
    module_state: &'a HashMap<String, LLVMValueRef>,
    module_runtime: &'a HashMap<String, (LLVMValueRef, LLVMTypeRef)>,
}

/// Log the input at debug level.
///
/// # Safety
/// The caller must ensure the pointer is valid and points to a null
/// terminated C-string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn debug(text: *const i8) -> u64 {
    let text = unsafe { CStr::from_ptr(text) };
    debug!("{}", text.to_str().unwrap());
    0
}

/// Open a file with create and write permissions and write to it.
///
/// # Safety
/// The caller must ensure the pointer is valid and points to a null
/// terminated C-string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn open_file(path: *const i8) -> u64 {
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

/// Connect to a specified address and send a text to it.
///
/// # Safety
/// The caller must ensure the pointer is valid and points to a null
/// terminated C-string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn ping(addr: *const i8) -> u64 {
    let addr = unsafe { CStr::from_ptr(addr).to_str().unwrap() };
    debug!("Ping {:?}", addr);
    let mut stream =
        TcpStream::connect(addr).expect("Couldn't connect to the server...");

    stream.write_all(b"hello\n").unwrap();

    // We expect "hello" string in return
    let mut buf = [0; 5];
    stream.read_exact(&mut buf).unwrap();

    stream
        .shutdown(Shutdown::Both)
        .expect("shutdown call failed");
    0
}

/// Spawn a process with a random argument.
///
/// # Safety
/// The caller must ensure the pointer is valid and points to a null
/// terminated C-string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn task(name: *const i8, args: *const i8) -> u64 {
    let name = unsafe { CStr::from_ptr(name) };

    let args = if !args.is_null() {
        debug!("Task {:?} {:?}", name, args);
        unsafe { CStr::from_ptr(args) }
    } else {
        debug!("Task {:?}, null", name);
        c""
    };

    Command::new(name.to_str().unwrap())
        .args(args.to_str().unwrap().split_whitespace())
        .status()
        .expect("Failed to execute task")
        .code()
        .unwrap_or(0)
        .try_into()
        .unwrap()
}

/// Listen on a specified number of ports starting from the lower boundary.
/// Open connections will live until the end of the block of work and will be
/// shutdown in cleanup instruction.
///
/// # Safety
/// The caller must ensure the pointer is valid and points to a null
/// terminated C-string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn listen_on_ports(lower: u64, n: u64) -> u64 {
    debug!("Listen {lower} {n}");
    let max_ports =
        Arc::clone(&MAX_PORTS).fetch_add(n as usize, Ordering::Relaxed);

    let start_port = lower + max_ports as u64;
    let _listeners: Vec<_> = (start_port..start_port + n)
        .map(|port| {
            let addr = format!("0.0.0.0:{port}");
            let listener = TcpListener::bind(&addr)
                .expect("Couldn't listen on the specified address");
            let fd = listener.as_raw_fd();

            trace!("Listen {addr}, fd {fd}");
            SOCKETS.with(|socks| socks.borrow_mut().push(fd));

            thread::spawn(move || for _stream in listener.incoming() {})
        })
        .collect();

    0
}

thread_local! {
    static POINTERS: RefCell<Vec<*mut i8>> = const { RefCell::new(vec![]) };
    static SOCKETS: RefCell<Vec<RawFd>> = const { RefCell::new(vec![]) };
}

pub static MAX_PORTS: LazyLock<Arc<AtomicUsize>> =
    LazyLock::new(|| Arc::new(AtomicUsize::new(0)));

/// Return a random integer from zipf distribution with specified
/// size and exponent.
///
/// # Safety
/// The caller must ensure the pointer is valid and points to a null
/// terminated C-string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn zipf(size: u64, exp: f64) -> u64 {
    debug!("zipf {size} {exp}");
    thread_rng().sample(Zipf::new(size, exp).unwrap()) as u64
}

/// Sleeps for specified amount of time.
///
/// # Safety
/// The caller must ensure the pointer is valid and points to a null
/// terminated C-string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn sleep(interval: f64) -> u64 {
    debug!("Sleep {interval}");
    thread::sleep(time::Duration::from_secs_f64(interval));
    0
}

/// Return a randomly generated string.
///
/// # Safety
/// The caller must ensure the pointer is valid and points to a null
/// terminated C-string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn random_string() -> *const i8 {
    let rand: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();

    let result = CString::new(rand).unwrap().into_raw();

    POINTERS.with(|ps| ps.borrow_mut().push(result));
    result
}

/// Return a randomly generated path.
///
/// # Safety
/// The caller must ensure the pointer is valid and points to a null
/// terminated C-string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn random_path(base: *const i8) -> *const i8 {
    let base = unsafe { CStr::from_ptr(base).to_string_lossy().into_owned() };

    let uniq: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();

    let result = CString::new(format!("{base}/{uniq}")).unwrap().into_raw();
    POINTERS.with(|ps| ps.borrow_mut().push(result));
    result
}

/// # Safety
#[unsafe(no_mangle)]
pub unsafe extern "C" fn cleanup(_: *const i8) -> u64 {
    debug!("Cleanup");
    POINTERS.with(|ps| {
        let mut vec = ps.borrow_mut();
        for p in vec.as_slice() {
            trace!("Cleanup {:?}", p);
            let _ = unsafe { CString::from_raw(*p) };
        }

        vec.clear();
    });

    SOCKETS.with(|socks| {
        let mut vec = socks.borrow_mut();
        for fd in vec.as_slice() {
            trace!("Shutdown {fd}");
            unsafe {
                libc::shutdown(*fd, libc::SHUT_RD);
            }
        }

        vec.clear();
    });

    0
}

#[derive(Debug, Clone)]
pub struct RuntimeFunc {
    func: usize,
    param_count: u32,
    param_types: &'static [RuntimeType],
    return_type: RuntimeType,
}

/// Functions, available in a script at runtime
pub static RUNTIME: LazyLock<HashMap<String, RuntimeFunc>> =
    LazyLock::new(|| {
        HashMap::from([
            // workload support
            (
                "task".to_string(),
                RuntimeFunc {
                    func: task as *const () as usize,
                    param_count: 2,
                    param_types: &[RuntimeType::Pointer, RuntimeType::Pointer],
                    return_type: RuntimeType::Int,
                },
            ),
            (
                "debug".to_string(),
                RuntimeFunc {
                    func: debug as *const () as usize,
                    param_count: 1,
                    param_types: &[RuntimeType::Pointer],
                    return_type: RuntimeType::Int,
                },
            ),
            (
                "open".to_string(),
                RuntimeFunc {
                    func: open_file as *const () as usize,
                    param_count: 1,
                    param_types: &[RuntimeType::Pointer],
                    return_type: RuntimeType::Int,
                },
            ),
            (
                "ping".to_string(),
                RuntimeFunc {
                    func: ping as *const () as usize,
                    param_count: 1,
                    param_types: &[RuntimeType::Pointer],
                    return_type: RuntimeType::Int,
                },
            ),
            (
                "listen".to_string(),
                RuntimeFunc {
                    func: listen_on_ports as *const () as usize,
                    param_count: 2,
                    param_types: &[RuntimeType::Int, RuntimeType::Int],
                    return_type: RuntimeType::Int,
                },
            ),
            (
                "sleep".to_string(),
                RuntimeFunc {
                    func: sleep as *const () as usize,
                    param_count: 1,
                    param_types: &[RuntimeType::Float],
                    return_type: RuntimeType::Pointer,
                },
            ),
            // dynamic values
            (
                "random_path".to_string(),
                RuntimeFunc {
                    func: random_path as *const () as usize,
                    param_count: 1,
                    param_types: &[RuntimeType::Pointer],
                    return_type: RuntimeType::Pointer,
                },
            ),
            (
                "random_string".to_string(),
                RuntimeFunc {
                    func: random_string as *const () as usize,
                    param_count: 0,
                    param_types: &[],
                    return_type: RuntimeType::Pointer,
                },
            ),
            (
                "zipf".to_string(),
                RuntimeFunc {
                    func: zipf as *const () as usize,
                    param_count: 2,
                    param_types: &[RuntimeType::Int, RuntimeType::Float],
                    return_type: RuntimeType::Pointer,
                },
            ),
            // utils
            (
                "cleanup".to_string(),
                RuntimeFunc {
                    func: cleanup as *const () as usize,
                    param_count: 1,
                    param_types: &[RuntimeType::Pointer],
                    return_type: RuntimeType::Int,
                },
            ),
        ])
    });

impl ScriptWorker {
    fn jit_instruction(name: &CStr, args: Vec<Arg>, ctx: &BuildContext) {
        let mut args_ptr = args
            .iter()
            .map(|a| Self::get_arg_value(a.clone(), ctx))
            .collect::<Vec<_>>();

        let (func, func_type) = ctx
            .module_runtime
            .get(name.to_str().expect("Couldn't convert name to string"))
            .unwrap();

        unsafe {
            LLVMBuildCall2(
                ctx.builder,
                *func_type,
                *func,
                args_ptr.as_mut_ptr(),
                args.len().try_into().unwrap(),
                name.as_ptr() as *const _,
            );
        }
    }

    fn get_arg_value(arg: Arg, ctx: &BuildContext) -> LLVMValueRef {
        match arg {
            Arg::Null => unsafe {
                let td = LLVMGetExecutionEngineTargetData(ctx.ee);
                let iptr = LLVMIntPtrTypeInContext(ctx.context, td);
                LLVMConstNull(iptr)
            },
            Arg::Const { value } => unsafe {
                match value {
                    ConstType::Text(text) => {
                        // The name of all constants created this way will be
                        // "const", which is ugly, but
                        // not a problem as LLVM modifies this to
                        // make sure uniqueness, i.e. they will be:
                        //
                        //      @const, @const.1, @const.2, ...
                        //
                        // in the jited code.
                        LLVMBuildGlobalString(
                            ctx.builder,
                            format!("{text}\0").as_ptr() as *const _,
                            c"const".as_ptr() as *const _,
                        )
                    }
                    ConstType::Int(value) => {
                        let i64t = LLVMInt64TypeInContext(ctx.context);
                        LLVMConstInt(i64t, value, 0)
                    }
                    ConstType::Float(value) => {
                        let double = LLVMDoubleTypeInContext(ctx.context);
                        LLVMConstReal(double, value)
                    }
                }
            },
            Arg::Var { name } => {
                *ctx.module_state.get(&name).expect("No variable")
            }
            Arg::Dynamic { name, args } => {
                let (func, func_type) = ctx
                    .module_runtime
                    .get(&name)
                    .expect("No dynamic variable in the runtime");

                let runtime_func = &RUNTIME
                    .get(&name)
                    .expect("No dynamic variable in the static runtime");

                let mut args_ptr = args
                    .iter()
                    .map(|a| Self::get_arg_value(a.clone(), ctx))
                    .collect::<Vec<_>>();

                unsafe {
                    trace!("Add mapping to {:?}", name);
                    let module_func = LLVMGetNamedFunction(
                        ctx.module,
                        format!("{name}\0").into_bytes().as_ptr() as *const _,
                    );

                    LLVMAddGlobalMapping(
                        ctx.ee,
                        module_func,
                        runtime_func.func as *mut c_void,
                    );

                    LLVMBuildCall2(
                        ctx.builder,
                        *func_type,
                        *func,
                        args_ptr.as_mut_ptr(),
                        args.len().try_into().unwrap(),
                        c"const".as_ptr() as *const _,
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
                    // In case of error, we must avoid using the uninitialized
                    // ExecutionEngineRef.
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
            let boolt = LLVMInt1TypeInContext(context);
            let float = LLVMFloatTypeInContext(context);
            let iptr = LLVMIntPtrTypeInContext(context, td);

            // Insert runtime functions into the module
            for (name, f) in RUNTIME.clone().into_iter() {
                if module_runtime.contains_key(&name) {
                    break;
                };

                let mut function_args = f
                    .param_types
                    .iter()
                    .map(|t| match t {
                        RuntimeType::Pointer => iptr,
                        RuntimeType::Int => i64t,
                        RuntimeType::Float => float,
                    })
                    .collect::<Vec<*mut LLVMType>>();

                let function_type = LLVMFunctionType(
                    match f.return_type {
                        RuntimeType::Int => i64t,
                        RuntimeType::Pointer => iptr,
                        RuntimeType::Float => float,
                    },
                    function_args.as_mut_ptr(),
                    f.param_count,
                    0,
                );

                let func = LLVMAddFunction(
                    module,
                    format!("{}\0", name).into_bytes().as_ptr() as *const _,
                    function_type,
                );
                debug!("Insert {} into runtime", name);
                module_runtime.insert(name.to_string(), (func, function_type));
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

            // Create a basic block in the function and set our builder to
            // generate code in it.
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

            let true_ptr = LLVMConstInt(boolt, 1, 0);
            module_state.insert(String::from("true"), true_ptr);

            let false_ptr = LLVMConstInt(boolt, 0, 0);
            module_state.insert(String::from("false"), false_ptr);

            let Node::Work {
                ref instructions, ..
            } = node
            else {
                unreachable!()
            };

            // Before JIT prepare a build context, we must have all the pieces
            // ready
            let ctx = BuildContext {
                ee,
                builder,
                module,
                context,
                module_state: &module_state,
                module_runtime: &module_runtime,
            };

            // Iterate proviled instructions and convert to JIT
            for instr in instructions {
                // JIT the instruction and collect it's name
                let name = match instr.clone() {
                    Instruction::Task { name, args } => {
                        let mut task_args = vec![name];
                        task_args.extend_from_slice(&args);

                        Self::jit_instruction(c"task", task_args, &ctx);
                        "task"
                    }

                    Instruction::Open { path } => {
                        Self::jit_instruction(c"open", vec![path], &ctx);
                        "open"
                    }

                    Instruction::Ping { server } => {
                        Self::jit_instruction(c"ping", vec![server], &ctx);
                        "ping"
                    }

                    Instruction::Debug { text } => {
                        Self::jit_instruction(c"debug", vec![text], &ctx);
                        "debug"
                    }

                    Instruction::Listen { lower, n } => {
                        Self::jit_instruction(c"listen", vec![lower, n], &ctx);
                        "listen"
                    }

                    Instruction::Sleep { interval } => {
                        Self::jit_instruction(c"sleep", vec![interval], &ctx);
                        "sleep"
                    }
                };

                // Populate the global mapping with observed runtime functions
                trace!("Add mapping to {:?}", name);

                let module_func = LLVMGetNamedFunction(
                    module,
                    format!("{name}\0").into_bytes().as_ptr() as *const _,
                );

                let runtime_func = &RUNTIME
                    .get(name)
                    .expect("No runtime function with the name");

                LLVMAddGlobalMapping(
                    ee,
                    module_func,
                    runtime_func.func as *mut c_void,
                );
            }

            // Final instruction to clear dangling pointers
            Self::jit_instruction(c"cleanup", vec![], &ctx);

            let module_func = LLVMGetNamedFunction(
                module,
                c"cleanup".to_bytes().as_ptr() as *const _,
            );

            let runtime_func = &RUNTIME
                .get("cleanup")
                .expect("No runtime function with the name");

            LLVMAddGlobalMapping(
                ee,
                module_func,
                runtime_func.func as *mut c_void,
            );

            // Emit a `ret i64` into the function to return the computed sum.
            let ret = LLVMConstInt(i64t, 0, 0);
            LLVMBuildRet(builder, ret);
            // done building
            LLVMDisposeBuilder(builder);

            if log_enabled!(Level::Debug) {
                // Dump the module as IR to stdout.
                LLVMDumpModule(module);
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
        let Node::Work { ref dist, .. } = self.node else {
            unreachable!()
        };

        match dist {
            Some(d) => {
                debug!("Distribution {:?}", d);
                let Dist::Exp { rate } = d else { todo!() };

                const MAX_CONCURRENT: usize = 16;
                let semaphore = Arc::new((
                    std::sync::Mutex::new(0usize),
                    std::sync::Condvar::new(),
                ));

                thread::scope(|s| {
                    loop {
                        {
                            let (lock, cvar) = &*semaphore;
                            let mut count = cvar
                                .wait_while(lock.lock().unwrap(), |c| {
                                    *c >= MAX_CONCURRENT
                                })
                                .unwrap();
                            *count += 1;
                        }

                        let worker = self.clone();
                        let sem = Arc::clone(&semaphore);
                        s.spawn(move || {
                            (worker.jit)();
                            let (lock, cvar) = &*sem;
                            *lock.lock().unwrap() -= 1;
                            cvar.notify_one();
                        });

                        let interval: f64 =
                            thread_rng().sample(Exp::new(*rate).unwrap());
                        debug!("Interval {}", interval);
                        thread::sleep(time::Duration::from_secs_f64(interval));
                    }
                });
            }
            None => {
                debug!("Single unit");
                (self.jit)();
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
