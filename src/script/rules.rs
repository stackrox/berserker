use log::debug;

use crate::script::ast::{Arg, Instruction, Node};
use std::collections::HashMap;

/// Apply following transformations to task instruction:
/// * If no arguments provided for the task, add an empty argument
fn apply_task_rules(task: &Instruction, _node: &Node) -> Instruction {
    let Instruction::Task { name, args } = task else {
        unreachable!()
    };

    let new_args = if args.is_empty() {
        vec![Arg::Null {}]
    } else {
        args.to_vec()
    };

    Instruction::Task {
        name: name.clone(),
        args: new_args,
    }
}

fn apply_instructions_rules(
    instructions: &[Instruction],
    node: &Node,
) -> Vec<Instruction> {
    instructions
        .iter()
        .map(|i| match i {
            Instruction::Task { .. } => apply_task_rules(i, node),
            _ => i.clone(),
        })
        .collect::<Vec<_>>()
        .to_vec()
}

fn apply_arg_rules(
    args: &HashMap<String, String>,
    _node: &Node,
) -> HashMap<String, String> {
    let mut new_args = args.clone();

    new_args.entry("workers".to_string()).or_insert_with(|| {
        debug!("Applying default number of workers");
        "1".to_string()
    });

    new_args.entry("duration".to_string()).or_insert_with(|| {
        debug!("Applying default duration");
        "0".to_string()
    });

    new_args
}

fn apply_work_rules(work: Node) -> Node {
    let Node::Work {
        ref name,
        ref args,
        ref instructions,
        ref dist,
    } = work
    else {
        unreachable!()
    };

    Node::Work {
        name: name.clone(),
        args: apply_arg_rules(args, &work),
        instructions: apply_instructions_rules(instructions, &work),
        dist: dist.clone(),
    }
}

fn apply_machine_rules(machine: Node) -> Node {
    machine
}

fn apply_node_rules(node: Node) -> Node {
    match node {
        Node::Work { .. } => apply_work_rules(node),
        Node::Machine { .. } => apply_machine_rules(node),
    }
}

/// Contains a list of transformation to apply after parsing.
/// Note that transformation does not update AST in place, but
/// rather provides an isolated copy of it. This may introduce
/// some parsing overhead of course, and has to be re-evaluated
/// every now and then.
///
/// TODO: Add following rules:
/// - add path if directory is expected
pub fn apply_rules(nodes: Vec<Node>) -> Vec<Node> {
    debug!("Applying rules");
    nodes.into_iter().map(apply_node_rules).collect::<Vec<_>>()
}
