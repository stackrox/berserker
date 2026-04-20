use crate::script::ast::MachineInstruction;

use log::{debug, trace};
use std::{
    fs,
    io::{BufReader, prelude::*},
    net::TcpListener,
    thread,
};

use crate::script::ast::Node;

#[derive(Debug)]
pub enum MachineError {
    Internal,
}

/// Start a dummy listening server
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
        thread::spawn(move || {
            loop {
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
            }
        });
    }

    Ok(())
}

/// Make sure a path exists
fn ensure_path(path: String) -> Result<(), MachineError> {
    debug!("Create path {path:?}");
    match fs::create_dir_all(path) {
        Ok(()) => {
            debug!("Success");
            Ok(())
        }
        Err(e) => {
            debug!("Failure {e:?}");
            Err(MachineError::Internal)
        }
    }
}

fn apply_instr(instr: MachineInstruction) -> Result<(), MachineError> {
    match instr {
        MachineInstruction::Server { port } => {
            start_server("127.0.0.1".to_string(), port)
        }
        MachineInstruction::Profile { target: _ } => todo!(),
        MachineInstruction::Path { value } => ensure_path(value),
    }
}

pub fn apply(machine: Vec<&Node>) -> Result<(), MachineError> {
    for m in machine {
        let Node::Machine {
            ref m_instructions, ..
        } = *m
        else {
            unreachable!()
        };

        for instr in m_instructions.clone() {
            // TODO: Note that for some machine statements there is a race
            // conditions here, since we do not wait for them to apply.
            // Introduce a distinction between background statements, and those
            // that have to be performed synchronously.
            thread::spawn(move || apply_instr(instr));
        }
    }

    Ok(())
}
