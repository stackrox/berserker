use log::debug;
use pest::{self, error::Error, Parser};
use std::collections::HashMap;

use crate::script::ast::{Arg, Dist, Instruction, MachineInstruction, Node};

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
    debug!("AST {:?}", ast);
    Ok(ast)
}

fn build_ast_from_expr(pair: pest::iterators::Pair<Rule>) -> Node {
    match pair.as_rule() {
        Rule::expr => build_ast_from_expr(pair.into_inner().next().unwrap()),
        Rule::machine => Node::Machine {
            m_instructions: build_ast_from_minstr(pair.into_inner()),
        },
        Rule::function => {
            let mut inner = pair.into_inner();
            let mut work = inner.next().unwrap().into_inner();
            let name = work.next().unwrap();
            let args_pair = work.next().unwrap().into_inner();
            let instructions = build_ast_from_instr(inner.next().unwrap());

            let dist = inner.next().map(build_ast_from_dist);
            let mut args: HashMap<String, String> = HashMap::new();

            for arg in args_pair {
                let mut inner = arg.into_inner();
                let key = inner.next().unwrap();
                let value = inner.next().unwrap();

                assert_eq!(key.as_rule(), Rule::ident);
                assert_eq!(value.as_rule(), Rule::value);

                args.insert(
                    key.as_span().as_str().to_string(),
                    value.as_span().as_str().to_string(),
                );
            }

            Node::Work {
                name: name.as_span().as_str().to_string(),
                args,
                instructions,
                dist,
            }
        }
        unknown => panic!("Unknown expr: {unknown:?}"),
    }
}

fn build_ast_from_minstr(
    pair: pest::iterators::Pairs<Rule>,
) -> Vec<MachineInstruction> {
    let mut instr = vec![] as Vec<MachineInstruction>;

    for i in pair {
        let mut inner = i.into_inner();
        let name = inner.next().unwrap();
        match name.into_inner().next().unwrap().as_rule() {
            Rule::server => {
                let port: u16 = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .next()
                    .unwrap()
                    .as_span()
                    .as_str()
                    .to_string()
                    .parse()
                    .unwrap();
                instr.push(MachineInstruction::Server { port });
            }
            Rule::profile => {
                let target = inner
                    .next()
                    .unwrap()
                    .into_inner()
                    .next()
                    .unwrap()
                    .as_span()
                    .as_str()
                    .to_string();
                instr.push(MachineInstruction::Profile { target });
            }
            unknown => panic!("Unknown machine instruction: {unknown:?}"),
        }
    }

    instr
}

fn build_ast_from_instr(pair: pest::iterators::Pair<Rule>) -> Vec<Instruction> {
    let mut instr = vec![] as Vec<Instruction>;

    for i in pair.into_inner() {
        let mut instrs = i.into_inner().next().unwrap().into_inner();
        let name = instrs.next().unwrap();

        let args_pair = instrs.next().unwrap().into_inner();
        let args: Vec<Arg> = args_pair
            .into_iter()
            .map(|arg| {
                let a = arg.into_inner().next().unwrap();
                match a.as_rule() {
                    Rule::constant => Arg::Const {
                        text: a
                            .into_inner()
                            .next()
                            .unwrap()
                            .as_span()
                            .as_str()
                            .to_string(),
                    },
                    Rule::ident => Arg::Var {
                        name: a.as_span().as_str().to_string(),
                    },
                    Rule::dynamic => {
                        let mut inner = a.into_inner();
                        let name = inner
                            .next()
                            .unwrap()
                            .as_span()
                            .as_str()
                            .to_string();
                        let args_pair = inner.next().unwrap().into_inner();
                        let args: Vec<Arg> = args_pair
                            .into_iter()
                            .map(|arg| {
                                let inner = arg.into_inner().next().unwrap();
                                let a = inner.into_inner().next().unwrap();
                                Arg::Const {
                                    text: a.as_span().as_str().to_string(),
                                }
                            })
                            .collect();

                        Arg::Dynamic { name, args }
                    }
                    unknown => panic!("Unknown arg type {unknown:?}"),
                }
            })
            .collect();

        match name.into_inner().next().unwrap().as_rule() {
            Rule::task => {
                instr.push(Instruction::Task {
                    name: args[0].clone(),
                    args,
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
                    let key =
                        inner.next().unwrap().as_span().as_str().to_string();
                    let value =
                        inner.next().unwrap().as_span().as_str().to_string();

                    opts.insert(key, value);
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

#[cfg(test)]
mod tests {
    use super::*;

    // Helper to verify repeated unit
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

    // Helper to verify random unit
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
}
