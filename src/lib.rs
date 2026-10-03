pub const BLOCK_SIZE: usize = 4096;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    overlay: Vec<Option<[u8; BLOCK_SIZE]>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Engine {
    base: Vec<[u8; BLOCK_SIZE]>,
    overlay: Vec<Option<[u8; BLOCK_SIZE]>>,
}

impl Engine {
    pub fn new(blocks: usize) -> Self {
        Self {
            base: vec![[0; BLOCK_SIZE]; blocks],
            overlay: vec![None; blocks],
        }
    }

    pub fn read(&self, index: usize) -> Option<[u8; BLOCK_SIZE]> {
        self.overlay
            .get(index)
            .and_then(|block| block.as_ref().copied())
            .or_else(|| self.base.get(index).copied())
    }

    pub fn write(&mut self, index: usize, block: [u8; BLOCK_SIZE]) {
        self.overlay[index] = Some(block);
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

    pub fn commit(&mut self) {
        for (base, change) in self.base.iter_mut().zip(self.overlay.iter_mut()) {
            if let Some(block) = change.take() {
                *base = block;
            }
        }
    }

    pub fn diff(&self, checkpoint: &Checkpoint) -> Vec<usize> {
        self.overlay
            .iter()
            .zip(checkpoint.overlay.iter())
            .enumerate()
            .filter_map(|(index, (current, saved))| {
                (current != saved).then_some(index)
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{Engine, BLOCK_SIZE};

    fn block(value: u8) -> [u8; BLOCK_SIZE] {
        [value; BLOCK_SIZE]
    }

    #[test]
    fn changes_are_visible_without_touching_base() {
        let mut engine = Engine::new(4);
        assert_eq!(engine.read(1), Some(block(0)));

        engine.write(1, block(7));
        assert_eq!(engine.read(1), Some(block(7)));
    }

    #[test]
    fn checkpoint_and_restore_revert_overlay_changes() {
        let mut engine = Engine::new(4);
        engine.write(1, block(1));

        let checkpoint = engine.checkpoint();
        engine.write(2, block(2));

        engine.restore(&checkpoint);

        assert_eq!(engine.read(1), Some(block(1)));
        assert_eq!(engine.read(2), Some(block(0)));
    }

    #[test]
    fn discard_removes_overlay_changes() {
        let mut engine = Engine::new(4);
        engine.write(3, block(3));

        engine.discard();

        assert_eq!(engine.read(3), Some(block(0)));
    }

    #[test]
    fn commit_moves_overlay_into_base() {
        let mut engine = Engine::new(4);
        engine.write(2, block(9));
        engine.commit();

        assert_eq!(engine.read(2), Some(block(9)));

        engine.write(2, block(8));
        let checkpoint = engine.checkpoint();
        engine.write(2, block(7));
        engine.restore(&checkpoint);

        assert_eq!(engine.read(2), Some(block(8)));
    }

    #[test]
    fn diff_reports_overlay_changes() {
        let mut engine = Engine::new(4);
        let checkpoint = engine.checkpoint();

        engine.write(0, block(5));
        engine.write(3, block(6));

        assert_eq!(engine.diff(&checkpoint), vec![0, 3]);
    }
}
