pub const BLOCK_SIZE: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block([u8; BLOCK_SIZE]);

impl Block {
    pub fn zeroed() -> Self {
        Self([0; BLOCK_SIZE])
    }

    pub fn from_bytes(bytes: [u8; BLOCK_SIZE]) -> Self {
        Self(bytes)
    }

    pub fn as_bytes(&self) -> &[u8; BLOCK_SIZE] {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BlockId(usize);

impl BlockId {
    pub fn new(index: usize) -> Self {
        Self(index)
    }

    pub fn index(self) -> usize {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    overlay: Vec<Option<Block>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Engine {
    base: Vec<Block>,
    overlay: Vec<Option<Block>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum WriteError {
    OutOfRange { id: BlockId, block_count: usize },
}

impl Engine {
    pub fn new(blocks: usize) -> Self {
        Self {
            base: vec![Block::zeroed(); blocks],
            overlay: vec![None; blocks],
        }
    }

    pub fn block_count(&self) -> usize {
        self.base.len()
    }

    pub fn read(&self, id: BlockId) -> Option<Block> {
        self.overlay
            .get(id.index())
            .and_then(|block| block.as_ref().cloned())
            .or_else(|| self.base.get(id.index()).cloned())
    }

    pub fn write(&mut self, id: BlockId, block: Block) -> Result<(), WriteError> {
        let index = id.index();

        if index >= self.block_count() {
            return Err(WriteError::OutOfRange {
                id,
                block_count: self.block_count(),
            });
        }

        self.overlay[index] = Some(block);
        Ok(())
    }

    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            overlay: self.overlay.clone(),
        }
    }

    pub fn restore(&mut self, checkpoint: &Checkpoint) {
        self.overlay = checkpoint.overlay.clone();
    }

    pub fn discard(&mut self) {
        self.overlay.fill(None);
    }

    pub fn dirty_count(&self) -> usize {
        self.overlay.iter().filter(|block| block.is_some()).count()
    }

    pub fn commit(&mut self) {
        for (base, change) in self.base.iter_mut().zip(self.overlay.iter_mut()) {
            if let Some(block) = change.take() {
                *base = block;
            }
        }
    }

    pub fn diff(&self, checkpoint: &Checkpoint) -> Vec<BlockId> {
        self.overlay
            .iter()
            .zip(checkpoint.overlay.iter())
            .enumerate()
            .filter_map(|(index, (current, saved))| {
                (current != saved).then_some(BlockId::new(index))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Block, BlockId, Engine, WriteError, BLOCK_SIZE};

    fn block(value: u8) -> Block {
        Block::from_bytes([value; BLOCK_SIZE])
    }

    fn id(index: usize) -> BlockId {
        BlockId::new(index)
    }

    #[test]
    fn changes_are_visible_through_overlay() {
        let mut engine = Engine::new(4);
        assert_eq!(engine.read(id(1)), Some(block(0)));

        engine.write(id(1), block(7)).unwrap();

        assert_eq!(engine.read(id(1)), Some(block(7)));
        assert_eq!(engine.dirty_count(), 1);
    }

    #[test]
    fn checkpoint_and_restore_revert_overlay_changes() {
        let mut engine = Engine::new(4);
        engine.write(id(1), block(1)).unwrap();

        let checkpoint = engine.checkpoint();
        engine.write(id(2), block(2)).unwrap();

        engine.restore(&checkpoint);

        assert_eq!(engine.read(id(1)), Some(block(1)));
        assert_eq!(engine.read(id(2)), Some(block(0)));
        assert_eq!(engine.dirty_count(), 1);
    }

    #[test]
    fn discard_removes_overlay_changes() {
        let mut engine = Engine::new(4);
        engine.write(id(3), block(3)).unwrap();

        engine.discard();

        assert_eq!(engine.read(id(3)), Some(block(0)));
        assert_eq!(engine.dirty_count(), 0);
    }

    #[test]
    fn commit_moves_overlay_into_base() {
        let mut engine = Engine::new(4);
        engine.write(id(2), block(9)).unwrap();
        engine.commit();

        assert_eq!(engine.read(id(2)), Some(block(9)));
        assert_eq!(engine.dirty_count(), 0);

        engine.write(id(2), block(8)).unwrap();
        let checkpoint = engine.checkpoint();
        engine.write(id(2), block(7)).unwrap();
        engine.restore(&checkpoint);

        assert_eq!(engine.read(id(2)), Some(block(8)));
    }

    #[test]
    fn diff_reports_overlay_changes() {
        let mut engine = Engine::new(4);
        let checkpoint = engine.checkpoint();

        engine.write(id(0), block(5)).unwrap();
        engine.write(id(3), block(6)).unwrap();

        assert_eq!(engine.diff(&checkpoint), vec![id(0), id(3)]);
    }

    #[test]
    fn out_of_range_write_is_rejected_without_panic() {
        let mut engine = Engine::new(4);

        let result = engine.write(id(4), block(9));

        assert_eq!(
            result,
            Err(WriteError::OutOfRange {
                id: id(4),
                block_count: 4,
            })
        );
        assert_eq!(engine.dirty_count(), 0);
    }

    #[test]
    fn out_of_range_read_returns_none() {
        let engine = Engine::new(4);

        assert_eq!(engine.read(id(4)), None);
    }
}
