use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Arg {
    Const { text: String },

    Var { name: String },
}

#[derive(Debug, Clone)]
pub enum Instruction {
    Task { name: String, args: Vec<Arg> },
    Open { path: String },
    Debug { text: String },
}

#[derive(Debug, Clone)]
pub enum MachineInstruction {
    Server { port: u16 },
}

#[derive(Debug, Clone)]
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
