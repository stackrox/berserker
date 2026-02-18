use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub enum Arg {
    /// Simple constant
    Const { text: String },

    /// Variable available at runtime
    Var { name: String },

    /// Helper like random_path
    Dynamic { name: String, args: Vec<Arg> },
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    Task { name: Arg, args: Vec<Arg> },
    Open { path: Arg },
    Debug { text: Arg },
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
