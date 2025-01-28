use std::{
    cmp,
    ffi::{c_char, CString},
    fmt::Display,
    mem, slice, thread,
};

use core_affinity::CoreId;
use libc::{SYS_bpf, SYS_perf_event_open};
use log::info;

use aya_obj::copy_instructions;
use aya_obj::generated::{
    bpf_attach_type, bpf_attr, bpf_cmd, bpf_prog_type, perf_event_attr,
    perf_event_sample_format, perf_type_id,
};

use crate::{BaseConfig, Worker, WorkerError, Workload, WorkloadConfig};

#[derive(Debug, Clone, Copy)]
pub struct BpfWorker {
    config: BaseConfig,
    workload: WorkloadConfig,
}

impl BpfWorker {
    pub fn new(workload: WorkloadConfig, cpu: CoreId, process: usize) -> Self {
        BpfWorker {
            config: BaseConfig { cpu, process },
            workload,
        }
    }
}

impl Worker for BpfWorker {
    fn run_payload(&self) -> Result<(), WorkerError> {
        info!("{self}");

        let Workload::Bpf { nprogs, tracepoint } = self.workload.workload
        else {
            unreachable!()
        };

        // Prepare the bpf program attributes
        let mut attr = unsafe { mem::zeroed::<bpf_attr>() };
        let u = unsafe { &mut attr.__bindgen_anon_3 };
        let mut prog_fd;
        let mut name: [c_char; 16] = [0; 16];

        // A simple two instruction BPF program, inspired by aya probes.
        let prog: &[u8] = &[
            0xb7, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            0x00, // mov64 r0 = 0
            0x95, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // exit
        ];

        let gpl = b"GPL\0";
        u.license = gpl.as_ptr() as u64;

        let insns = copy_instructions(prog).unwrap();
        u.insn_cnt = insns.len() as u32;
        u.insns = insns.as_ptr() as u64;
        // TODO: Extend for more target types
        u.prog_type = bpf_prog_type::BPF_PROG_TYPE_TRACEPOINT as u32;

        // Prepare the perf event attribute to find the attachment target
        let mut perf_attr = unsafe { mem::zeroed::<perf_event_attr>() };
        let mut perf_event_fd;

        perf_attr.config = tracepoint;
        perf_attr.size = mem::size_of::<perf_event_attr>() as u32;
        perf_attr.type_ = perf_type_id::PERF_TYPE_TRACEPOINT as u32;
        perf_attr.sample_type =
            perf_event_sample_format::PERF_SAMPLE_RAW as u64;
        perf_attr.set_inherit(0);

        // Prepare the bpf link attribute
        let mut link_attr = unsafe { mem::zeroed::<bpf_attr>() };
        link_attr.link_create.attach_type =
            bpf_attach_type::BPF_PERF_EVENT as u32;

        for i in 0..nprogs {
            let cstring = CString::new(format!("berserker{i}")).unwrap();
            let name_bytes = cstring.to_bytes();
            let len = cmp::min(name.len(), name_bytes.len());
            name[..len].copy_from_slice(unsafe {
                slice::from_raw_parts(name_bytes.as_ptr() as *const c_char, len)
            });
            attr.__bindgen_anon_3.prog_name = name;

            // Load the BPF program
            unsafe {
                prog_fd = libc::syscall(
                    SYS_bpf,
                    bpf_cmd::BPF_PROG_LOAD,
                    &attr,
                    mem::size_of::<bpf_attr>(),
                );
            }

            // Now prepare a tracepoint event the bpf program
            // will be attached to
            unsafe {
                perf_event_fd = libc::syscall(
                    SYS_perf_event_open,
                    &perf_attr,
                    0,
                    -1,
                    -1,
                    0,
                );
            }

            // And finally create a link between the program
            // and the tracepoint
            link_attr.link_create.__bindgen_anon_1.prog_fd = prog_fd as u32;
            link_attr.link_create.__bindgen_anon_2.target_fd =
                perf_event_fd as u32;
            unsafe {
                libc::syscall(
                    SYS_bpf,
                    bpf_cmd::BPF_LINK_CREATE,
                    &link_attr,
                    mem::size_of::<bpf_attr>(),
                );
            }
        }

        // We don't recreate those programs, and just let them live
        // for the duration of the run, blocking the main loop.
        loop {
            thread::park();
        }
    }
}

impl Display for BpfWorker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.config)
    }
}
