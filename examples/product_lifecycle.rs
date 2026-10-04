use mse_core::{Block, BlockId, Engine, BLOCK_SIZE};

fn block(value: u8) -> Block {
    Block::from_bytes([value; BLOCK_SIZE])
}

fn value(engine: &Engine, index: usize) -> u8 {
    engine
        .read(BlockId::new(index))
        .expect("scenario block is in range")
        .as_bytes()[0]
}

fn main() {
    let mut engine = Engine::new(3);
    println!("created: blocks={}", engine.block_count());

    engine.write(BlockId::new(0), block(10)).unwrap();
    println!("write: block=0 value={}", value(&engine, 0));

    let checkpoint = engine.checkpoint();
    println!("checkpoint: created");

    engine.write(BlockId::new(0), block(20)).unwrap();
    engine.write(BlockId::new(1), block(30)).unwrap();
    let changed: Vec<_> = engine
        .diff(&checkpoint)
        .unwrap()
        .into_iter()
        .map(BlockId::index)
        .collect();
    assert_eq!(changed, vec![0, 1]);
    println!("diff: blocks={changed:?}");

    engine.restore(&checkpoint).unwrap();
    assert_eq!(value(&engine, 0), 10);
    assert_eq!(value(&engine, 1), 0);
    println!(
        "restore: block0={} block1={}",
        value(&engine, 0),
        value(&engine, 1)
    );

    engine.commit();
    assert_eq!(engine.dirty_count(), 0);
    assert_eq!(value(&engine, 0), 10);
    println!(
        "commit: dirty={} block0={}",
        engine.dirty_count(),
        value(&engine, 0)
    );

    engine.write(BlockId::new(1), block(40)).unwrap();
    engine.write(BlockId::new(2), block(50)).unwrap();
    engine.discard();
    assert_eq!(value(&engine, 0), 10);
    assert_eq!(value(&engine, 1), 0);
    assert_eq!(value(&engine, 2), 0);
    assert_eq!(engine.dirty_count(), 0);
    println!(
        "discard: dirty={} blocks=[{}, {}, {}]",
        engine.dirty_count(),
        value(&engine, 0),
        value(&engine, 1),
        value(&engine, 2)
    );
}
