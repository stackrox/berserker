use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    /// Null constant
    Null {},

    /// Simple constant
    Const { text: String },

    /// Variable available at runtime
    Var { name: String },

    /// Helper like random_path
    Dynamic { name: String, args: Vec<Arg> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    /// Execute a binary with specified name and arguments
    Task { name: Arg, args: Vec<Arg> },

    /// Open a file at specified path
    Open { path: Arg },

    /// Print a debugging message (subject to configured log level)
    Debug { text: Arg },

    /// Send a message to a server at specified address
    Ping { server: Arg },
}

#[derive(Debug, Clone, PartialEq)]
pub enum MachineInstruction {
    Server { port: u16 },
    Profile { target: String },
    Path { value: String },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Dist {
    Exp { rate: f64 },
    Zipf { frequency: f64, exponent: f64 },
    Uniform { upper: f64, lower: f64 },
}

#[derive(Debug, Clone)]
pub enum Node {
    Machine {
        m_instructions: Vec<MachineInstruction>,
    },
    Work {
        name: String,
        args: HashMap<String, String>,
        instructions: Vec<Instruction>,
        dist: Option<Dist>,
    },
}
