use log::debug;
use pest::{self, error::Error, Parser};
use std::collections::HashMap;

use crate::script::ast::{Arg, Dist, Instruction, Node};

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
    for pair in pairs {
        if let Rule::file = pair.as_rule() {
            ast.push(build_ast_from_expr(pair.into_inner().next().unwrap()));
        }
    }
    debug!("AST {:?}", ast);
    Ok(ast)
}

fn build_ast_from_expr(pair: pest::iterators::Pair<Rule>) -> Node {
    match pair.as_rule() {
        Rule::expr => build_ast_from_expr(pair.into_inner().next().unwrap()),
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

fn build_ast_from_instr(pair: pest::iterators::Pair<Rule>) -> Vec<Instruction> {
    let mut instr = vec![] as Vec<Instruction>;

    for i in pair.into_inner() {
        let mut instrs = i.into_inner().next().unwrap().into_inner();
        let name = instrs.next().unwrap().as_span().as_str().to_string();
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
                    unknown => panic!("Unknown arg type {unknown:?}"),
                }
            })
            .collect();

        instr.push(Instruction::Task { name, args });
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
