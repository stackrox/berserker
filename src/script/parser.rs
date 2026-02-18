use log::{debug, trace};
use pest::{self, Parser, error::Error};
use std::collections::HashMap;

use crate::script::ast::{Arg, Dist, Instruction, MachineInstruction, Node};

#[derive(Debug)]
pub enum ParseError {
    NotSupported,
    TypeMismatch,
}

// ANCHOR: parser
#[derive(pest_derive::Parser)]
#[grammar = "script/grammar.peg"]
struct InstructionParser;
// ANCHOR_END: parser

// ANCHOR: parse_source
// Rule can be large depending on the grammar, and we don't really control
// this. Thus ignore clippy warning about the large error.
#[allow(clippy::result_large_err)]
pub fn parse_instructions(source: &str) -> Result<Vec<Node>, Error<Rule>> {
    pest::set_error_detail(true);
    let mut ast = vec![];
    let pairs = InstructionParser::parse(Rule::file, source)?;
    let expr_rules = [Rule::expr, Rule::machine];

    for pair in pairs {
        if pair.as_rule() != Rule::file {
            continue;
        }

        for i in pair.into_inner() {
            if expr_rules.contains(&i.as_rule()) {
                ast.push(build_ast_from_expr(i));
            }
        }
    }
    trace!("Resulting AST: {:?}", ast);
    Ok(ast)
}

fn build_ast_from_expr(pair: pest::iterators::Pair<Rule>) -> Node {
    match pair.as_rule() {
        Rule::expr => build_ast_from_expr(pair.into_inner().next().unwrap()),
        Rule::machine => Node::Machine {
            m_instructions: build_ast_from_minstr(pair.into_inner()),
        },
        Rule::function => build_ast_from_function(pair.into_inner()),
        unknown => panic!("Unknown expr: {unknown:?}"),
    }
}

fn build_ast_from_minstr(
    pair: pest::iterators::Pairs<Rule>,
) -> Vec<MachineInstruction> {
    let mut instr = vec![] as Vec<MachineInstruction>;

    for i in pair {
        let mut inner = i.into_inner();
        let name = inner.next().expect("No instruction name");

        match first_nested_pair(name).as_rule() {
            Rule::server => {
                let port_pair =
                    first_nested_pair(inner.next().expect("No port"));
                let port: u16 = pair_to_string(port_pair)
                    .parse()
                    .expect("Cannot parse port");
                instr.push(MachineInstruction::Server { port });
            }
            Rule::profile => {
                let target =
                    first_nested_pair(inner.next().expect("No target"));
                instr.push(MachineInstruction::Profile {
                    target: pair_to_string(target),
                });
            }
            Rule::path => {
                let first_arg =
                    first_nested_pair(inner.next().expect("No path"));

                let value = match string_from_argument(first_arg) {
                    Ok(value) => value,
                    Err(e) => panic!("Cannot parse argument: {e:?}"),
                };

                instr.push(MachineInstruction::Path { value });
            }
            unknown => panic!("Unknown machine instruction: {unknown:?}"),
        }
    }

    instr
}

fn build_ast_from_work(
    pairs: pest::iterators::Pairs<Rule>,
) -> (String, HashMap<String, String>) {
    let mut work_parts = pairs.clone();

    let name = work_parts.next().expect("No work name");
    let params = work_parts.next().expect("Wo work parameters");

    let name_str = pair_to_string(name);
    let mut params_map: HashMap<String, String> = HashMap::new();

    for param in params.into_inner() {
        let mut kv = param.into_inner();
        let key = kv.next().expect("No parameter name");
        let value = kv.next().expect("No parameter value");

        assert_eq!(key.as_rule(), Rule::ident);
        assert_eq!(value.as_rule(), Rule::value);

        params_map.insert(pair_to_string(key), pair_to_string(value));
    }

    (name_str, params_map)
}

fn build_ast_from_function(pairs: pest::iterators::Pairs<Rule>) -> Node {
    let mut func_parts = pairs.clone();

    let work = func_parts.next().expect("No work unit");
    let instrs = func_parts.next().expect("No instructions");
    let distribution = func_parts.next();

    let (name, args) = build_ast_from_work(work.into_inner());
    let instructions = build_ast_from_instr(instrs.into_inner());
    let dist = distribution.map(build_ast_from_dist);

    Node::Work {
        name,
        args,
        instructions,
        dist,
    }
}

fn build_ast_from_instr(
    pairs: pest::iterators::Pairs<Rule>,
) -> Vec<Instruction> {
    let mut instr = vec![] as Vec<Instruction>;

    for pair in pairs {
        let mut instrs = first_nested_pair(pair).into_inner();

        let name = instrs.next().expect("No instruction name");
        let args_pair = instrs.next().expect("No instruction arguments");

        let args: Vec<Arg> = args_pair
            .into_inner()
            .into_iter()
            .map(|arg| {
                let a = first_nested_pair(arg);
                match a.as_rule() {
                    Rule::constant => Arg::Const {
                        text: pair_to_string(first_nested_pair(a)),
                    },
                    Rule::ident => Arg::Var {
                        name: pair_to_string(a),
                    },
                    Rule::dynamic => {
                        let mut inner = a.into_inner();
                        let name = inner.next().expect("No argument name");
                        let args_pair =
                            inner.next().expect("No argument value");

                        let args: Vec<Arg> = args_pair
                            .into_inner()
                            .into_iter()
                            .map(|arg| {
                                let a =
                                    first_nested_pair(first_nested_pair(arg));
                                Arg::Const {
                                    text: pair_to_string(a),
                                }
                            })
                            .collect();

                        Arg::Dynamic {
                            name: pair_to_string(name),
                            args,
                        }
                    }
                    unknown => panic!("Unknown arg type {unknown:?}"),
                }
            })
            .collect();

        match first_nested_pair(name).as_rule() {
            Rule::task => {
                let Some((name, arg_list)) = args.split_first() else {
                    unreachable!()
                };

                instr.push(Instruction::Task {
                    name: name.clone(),
                    args: arg_list.to_vec(),
                });
            }
            Rule::open => {
                instr.push(Instruction::Open {
                    path: args[0].clone(),
                });
            }
            Rule::debug => {
                instr.push(Instruction::Debug {
                    text: args[0].clone(),
                });
            }
            Rule::ping => {
                instr.push(Instruction::Ping {
                    server: args[0].clone(),
                });
            }
            unknown => panic!("Unknown instruction type {unknown:?}"),
        }
    }

    instr
}

fn build_ast_from_dist(pair: pest::iterators::Pair<Rule>) -> Dist {
    match pair.as_rule() {
        Rule::dist => {
            let mut opts: HashMap<String, String> = HashMap::new();

            for p in pair.into_inner() {
                if let Rule::opt = p.as_rule() {
                    let mut inner = p.into_inner();
                    let key = inner.next().expect("No dist argument key");
                    let value = inner.next().expect("No dist argument value");

                    opts.insert(pair_to_string(key), pair_to_string(value));
                }
            }

            Dist::Exp {
                rate: opts
                    .get("rate")
                    .cloned()
                    .unwrap_or(String::from("0"))
                    .parse()
                    .unwrap(),
            }
        }
        unknown => panic!("Unknown dist: {unknown:?}"),
    }
}

fn string_from_constant(pair: pest::iterators::Pair<Rule>) -> String {
    assert_eq!(pair.as_rule(), Rule::constant);

    // Extract "value" rule and convert it to String
    pair_to_string(first_nested_pair(pair))
}

fn string_from_ident(pair: pest::iterators::Pair<Rule>) -> String {
    assert_eq!(pair.as_rule(), Rule::ident);

    // Extract "name" rule and convert it to String
    pair_to_string(first_nested_pair(pair))
}

fn string_from_argument(
    pair: pest::iterators::Pair<Rule>,
) -> Result<String, ParseError> {
    assert_eq!(pair.as_rule(), Rule::arg);

    let inner = first_nested_pair(pair);
    match inner.as_rule() {
        Rule::constant => Ok(string_from_constant(inner)),
        Rule::ident => Ok(string_from_ident(inner)),
        Rule::dynamic => Err(ParseError::NotSupported),
        _ => Err(ParseError::TypeMismatch),
    }
}

fn pair_to_string(pair: pest::iterators::Pair<Rule>) -> String {
    pair.as_span().as_str().to_string()
}

fn first_nested_pair(
    pair: pest::iterators::Pair<Rule>,
) -> pest::iterators::Pair<Rule> {
    pair.into_inner().next().expect("Cannot get first pair")
}

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to verify a repeated unit
    fn test_repeated(node: Node) {
        let Node::Work {
            ref name,
            ref args,
            ref instructions,
            ref dist,
        } = node
        else {
            unreachable!()
        };

        assert_eq!(name, "repeated");

        assert_eq!(args.len(), 2);
        assert_eq!(args.get("workers").unwrap(), "10");
        assert_eq!(args.get("duration").unwrap(), "100");

        assert_eq!(instructions.len(), 1);

        assert_eq!(
            instructions[0],
            Instruction::Open {
                path: Arg::Const {
                    text: "/tmp/test".to_string()
                }
            }
        );

        let dist_value = dist.clone().unwrap();

        assert_eq!(dist_value, Dist::Exp { rate: 100.0 });
    }

    // Helper to verify a random unit
    fn test_random(node: Node) {
        let Node::Work {
            ref name,
            ref args,
            ref instructions,
            ref dist,
        } = node
        else {
            unreachable!()
        };

        assert_eq!(name, "random");

        assert_eq!(args.len(), 2);
        assert_eq!(args.get("workers").unwrap(), "10");
        assert_eq!(args.get("duration").unwrap(), "100");

        assert_eq!(instructions.len(), 1);

        assert_eq!(
            instructions[0],
            Instruction::Open {
                path: Arg::Dynamic {
                    name: "random_path".to_string(),
                    args: vec![Arg::Const {
                        text: "/tmp".to_string()
                    }],
                }
            }
        );

        let dist_value = dist.clone().unwrap();

        assert_eq!(dist_value, Dist::Exp { rate: 100.0 });
    }

    // Helper to verify a task unit
    fn test_task(node: Node, global_opts: bool, exp: bool) {
        let Node::Work {
            ref name,
            ref args,
            ref instructions,
            ref dist,
        } = node
        else {
            unreachable!()
        };

        assert_eq!(name, "main");

        if global_opts {
            assert_eq!(args.len(), 2);
            assert_eq!(args.get("workers").unwrap(), "2");
            assert_eq!(args.get("duration").unwrap(), "10");
        }

        assert_eq!(instructions.len(), 2);

        assert_eq!(
            instructions[0],
            Instruction::Debug {
                text: Arg::Const {
                    text: "run task stub".to_string(),
                }
            }
        );

        assert_eq!(
            instructions[1],
            Instruction::Task {
                name: Arg::Var {
                    name: "stub".to_string(),
                },
                args: vec![],
            }
        );

        if exp {
            let dist_value = dist.clone().unwrap();

            assert_eq!(dist_value, Dist::Exp { rate: 10.0 });
        }
    }

    // Helper to verify a ping unit
    fn test_ping(node: Node, global_opts: bool, exp: bool) {
        let Node::Work {
            ref name,
            ref args,
            ref instructions,
            ref dist,
        } = node
        else {
            unreachable!()
        };

        assert_eq!(name, "main");

        if global_opts {
            assert_eq!(args.len(), 2);
            assert_eq!(args.get("workers").unwrap(), "2");
            assert_eq!(args.get("duration").unwrap(), "10");
        }

        assert_eq!(instructions.len(), 2);

        assert_eq!(
            instructions[0],
            Instruction::Debug {
                text: Arg::Const {
                    text: "ping server".to_string(),
                }
            }
        );

        assert_eq!(
            instructions[1],
            Instruction::Ping {
                server: Arg::Const {
                    text: "127.0.0.1:8080".to_string(),
                },
            }
        );

        if exp {
            let dist_value = dist.clone().unwrap();

            assert_eq!(dist_value, Dist::Exp { rate: 10.0 });
        }
    }

    // Helper to verify a machine unit
    fn test_machine(node: Node, server: bool, profile: bool) {
        let Node::Machine { ref m_instructions } = node else {
            unreachable!()
        };

        assert_eq!(m_instructions.len(), 1);

        if server {
            assert_eq!(
                m_instructions[0],
                MachineInstruction::Server { port: 8080 }
            );
        }

        if profile {
            assert_eq!(
                m_instructions[0],
                MachineInstruction::Profile {
                    target: "bpf".to_string(),
                }
            );
        }
    }

    #[test]
    fn test_single_work_unit() {
        let input = r#"
            // open the same file over and over
            repeated (workers = 10, duration = 100) {
              open("/tmp/test");
            } : exp {
              rate = 100.0;
            }
        "#;

        let ast: Vec<Node> = parse_instructions(input).unwrap();
        assert_eq!(ast.len(), 1);

        test_repeated(ast[0].clone());
    }

    #[test]
    fn test_multiple_work_units() {
        let input = r#"
            // open lots of random files under
            // a specified directory
            random (workers = 10, duration = 100) {
              open(random_path("/tmp"));
            } : exp {
              rate = 100.0;
            }

            // open the same file over and over
            repeated (workers = 10, duration = 100) {
              open("/tmp/test");
            } : exp {
              rate = 100.0;
            }
        "#;

        let ast: Vec<Node> = parse_instructions(input).unwrap();
        assert_eq!(ast.len(), 2);

        test_random(ast[0].clone());
        test_repeated(ast[1].clone());
    }

    #[test]
    fn test_task_unit() {
        let input = r#"
            // Named work block
            main (workers = 2, duration = 10) {
              // Anon work block with only one unit.
              // task(name) -- spawn a process with specified name
              // debug(text) -- log with DEBUG level
              // open(path) -- open file by path, create if needed and write something to it
              debug("run task stub");
              task(stub);
            } : exp {
              // If no distribution provided, do the unit only once.
              rate = 10.0;
            }
        "#;

        let ast: Vec<Node> = parse_instructions(input).unwrap();
        assert_eq!(ast.len(), 1);

        test_task(ast[0].clone(), true, true);
    }

    #[test]
    fn test_task_unit_no_dist() {
        let input = r#"
            main () {
              debug("run task stub");
              task(stub);
            }
        "#;

        let ast: Vec<Node> = parse_instructions(input).unwrap();
        assert_eq!(ast.len(), 1);

        test_task(ast[0].clone(), false, false);
    }

    #[test]
    fn test_ping_with_machine() {
        let input = r#"
            machine {
              server(8080);
            }

            main () {
              debug("ping server");
              ping("127.0.0.1:8080");
            }
        "#;

        let ast: Vec<Node> = parse_instructions(input).unwrap();
        assert_eq!(ast.len(), 2);

        test_machine(ast[0].clone(), true, false);
        test_ping(ast[1].clone(), false, false);
    }
}
