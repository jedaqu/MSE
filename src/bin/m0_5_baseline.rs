use mse_core::{Block, BlockId, Engine};
use std::hint::black_box;
use std::time::Instant;

const BLOCKS: usize = 256;
const WARMUP: usize = 1_000;
const ITERATIONS: usize = 10_000;

fn block(value: u8) -> Block {
    Block::from_bytes([value; mse_core::BLOCK_SIZE])
}

fn report(name: &str, iterations: usize, duration: std::time::Duration) {
    let total_ns = duration.as_nanos();
    let per_op = total_ns as f64 / iterations as f64;
    println!("{name}: iterations={iterations} total_ns={total_ns} ns_per_op={per_op:.2}");
}

fn main() {
    let mut engine = Engine::new(BLOCKS);
    let id = BlockId::new(1);
    engine.write(id, block(7)).unwrap();

    for _ in 0..WARMUP {
        black_box(engine.read(id));
    }
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        black_box(engine.read(id));
    }
    report("read", ITERATIONS, start.elapsed());

    let mut engine = Engine::new(BLOCKS);
    for iteration in 0..WARMUP {
        let value = black_box((iteration as u8).wrapping_add(1));
        engine.write(id, block(value)).unwrap();
    }
    let start = Instant::now();
    for iteration in 0..ITERATIONS {
        let value = black_box((iteration as u8).wrapping_add(1));
        engine.write(id, block(value)).unwrap();
    }
    black_box(engine.read(id));
    report("write", ITERATIONS, start.elapsed());

    let mut engine = Engine::new(BLOCKS);
    engine.write(id, block(3)).unwrap();
    for _ in 0..WARMUP {
        black_box(engine.checkpoint());
    }
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        black_box(engine.checkpoint());
    }
    report("checkpoint", ITERATIONS, start.elapsed());

    let mut engine = Engine::new(BLOCKS);
    let checkpoint = engine.checkpoint();
    engine.write(id, block(5)).unwrap();
    for _ in 0..WARMUP {
        black_box(engine.diff(&checkpoint).unwrap());
    }
    let start = Instant::now();
    for _ in 0..ITERATIONS {
        black_box(engine.diff(&checkpoint).unwrap());
    }
    report("diff", ITERATIONS, start.elapsed());

    let mut engine = Engine::new(BLOCKS);
    engine.write(id, block(6)).unwrap();
    let checkpoint = engine.checkpoint();
    for iteration in 0..WARMUP {
        let value = black_box((iteration as u8).wrapping_add(2));
        engine.write(id, block(value)).unwrap();
        engine.restore(&checkpoint).unwrap();
    }
    let start = Instant::now();
    for iteration in 0..ITERATIONS {
        let value = black_box((iteration as u8).wrapping_add(2));
        engine.write(id, block(value)).unwrap();
        engine.restore(&checkpoint).unwrap();
    }
    black_box(engine.read(id));
    report("write+restore", ITERATIONS, start.elapsed());

    let mut engine = Engine::new(BLOCKS);
    for iteration in 0..WARMUP {
        let value = black_box((iteration as u8).wrapping_add(3));
        engine.write(id, block(value)).unwrap();
        engine.commit();
    }
    let start = Instant::now();
    for iteration in 0..ITERATIONS {
        let value = black_box((iteration as u8).wrapping_add(3));
        engine.write(id, block(value)).unwrap();
        engine.commit();
    }
    black_box(engine.read(id));
    report("write+commit", ITERATIONS, start.elapsed());
}
