use crate::script::ast::MachineInstruction;

use log::{debug, trace};
use std::{
    io::{prelude::*, BufReader},
    mem,
    net::TcpListener,
    thread,
};

use libc::SYS_bpf;

use aya::{include_bytes_aligned, programs::FEntry, Btf, Ebpf, Endianness};
use aya_obj::generated::{bpf_attr, bpf_btf_info, bpf_cmd};

#[derive(Debug)]
pub enum MachineError {
    Internal,
}

fn start_server(addr: String, target_port: u16) -> Result<(), MachineError> {
    debug!("Starting server at {:?}:{:?}", addr, target_port);

    let listener = TcpListener::bind((addr, target_port)).unwrap();

    for stream in listener.incoming() {
        let mut stream = stream.unwrap();

        // As a simplest solution to keep a connection open, spawn a
        // thread.  It's not the best one though, as we waste resources.
        // For the purpose of only keeping connections open we could e.g.
        // spawn only two threads, where the first one receives connections
        // and adds streams into the list of active, and the second iterates
        // through streams and replies. This way the connections will have
        // high latency, but for the purpose of networking workload it
        // doesn't matter.
        thread::spawn(move || loop {
            let mut buf_reader = BufReader::new(&stream);
            let mut buffer = String::new();

            match buf_reader.read_line(&mut buffer) {
                Ok(0) => {
                    // EOF, exit
                    trace!("EOF");
                    return;
                }
                Ok(_n) => {
                    trace!("Received {:?}", buffer);

                    let response = "hello\n";
                    match stream.write_all(response.as_bytes()) {
                        Ok(_) => {
                            // Response is sent, handle the next one
                        }
                        Err(e) => {
                            trace!("ERROR: sending response, {}", e);
                            break;
                        }
                    }
                }
                Err(e) => {
                    trace!("ERROR: reading a line, {}", e)
                }
            }
        });
    }

    Ok(())
}

fn start_bpf_profiling() -> Result<(), MachineError> {
    debug!("Starting eBPF profiling");
    let btf_fd;
    let mut buf = vec![0u8; 4096];

    // Load the BPF program
    unsafe {
        let mut fd_attr = mem::zeroed::<bpf_attr>();
        fd_attr.__bindgen_anon_6.__bindgen_anon_1.btf_id = 293;

        btf_fd = libc::syscall(
            SYS_bpf,
            bpf_cmd::BPF_BTF_GET_FD_BY_ID,
            &fd_attr,
            mem::size_of::<bpf_attr>(),
        );

        let mut info_attr = mem::zeroed::<bpf_attr>();
        let mut info = mem::zeroed::<bpf_btf_info>();

        info.btf = buf.as_mut_ptr() as _;
        info.btf_size = buf.len() as _;

        info_attr.info.bpf_fd = btf_fd as u32;
        info_attr.info.info = &info as *const _ as u64;
        info_attr.info.info_len = mem::size_of_val(&info) as u32;

        libc::syscall(
            SYS_bpf,
            bpf_cmd::BPF_OBJ_GET_INFO_BY_FD,
            &info_attr,
            mem::size_of::<bpf_attr>(),
        );
    }

    let btf = Btf::parse(&buf, Endianness::default()).unwrap();

    debug!("Got btf {:?}", btf);

    debug!("Loading eBPF program");
    let mut bpf =
        match Ebpf::load(include_bytes_aligned!("../bpf/fentry.bpf.o")) {
            Ok(prog) => {
                debug!("Loaded prog");
                prog
            }
            Err(e) => {
                panic!("Cannot load eBPF program, {e}");
            }
        };

    debug!("Loaded eBPF program");
    //let btf = Btf::from_sys_fs().unwrap();
    let btf = match Btf::parse_file("/tmp/btf", Endianness::default()) {
        Ok(data) => data,
        Err(e) => panic!("Cannot parse BTF {e}"),
    };
    let fentry: &mut FEntry =
        bpf.program_mut("fentry_XXX").unwrap().try_into().unwrap();

    fentry.load("handle_tp", &btf).unwrap();
    debug!("Loaded fentry program");

    fentry.attach().unwrap();
    debug!("Attached fentry");

    Ok(())
}

pub fn apply(instr: MachineInstruction) -> Result<(), MachineError> {
    match instr {
        MachineInstruction::Server { port } => {
            start_server("127.0.0.1".to_string(), port)
        }
        MachineInstruction::Profile { target: _ } => start_bpf_profiling(),
    }
}
