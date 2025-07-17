#![feature(test)]

extern crate test;

use lib::frontend::create_graph_from_ebnf;
use std::fs::read_to_string;
use test::Bencher;

#[bench]
fn benchmark(b: &mut Bencher) {
    let ebnf = read_to_string("../js.ebnf").unwrap();
    b.iter(|| create_graph_from_ebnf(&ebnf));
}
