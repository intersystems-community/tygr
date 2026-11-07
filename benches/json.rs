//! Parse throughput of the JSON grammar shared with `examples/json.rs`.
//!
//!   cargo bench --bench json

use std::hint::black_box;
use std::time::Instant;

use tygr::Grammar;

#[path = "../examples/common/json.rs"]
mod json;

use json::Json;

const CANADA: &str = include_str!("../data/canada.json");
const DATA: &str = include_str!("../data/data.json");

fn bench(label: &str, input: &str) {
    let mb = input.len() as f64 / 1e6;
    let mut best = f64::MAX;
    for _ in 0..30 {
        let start = Instant::now();
        let v = Json::parse(input).unwrap();
        best = best.min(start.elapsed().as_secs_f64());
        black_box(&v);
    }
    println!("{label}: {:.3}ms  {:.1} MB/s", best * 1e3, mb / best);
}

fn main() {
    bench("CANADA", CANADA);
    bench("DATA", DATA);
}
