pub const BLOCK_SIZE: usize = 4096;

use std::collections::HashMap;

static NEXT_CHECKPOINT_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NEXT_OBJECT_STORE_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
static NEXT_TRANSACTION_ID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
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

/// Process-local identity for one in-memory object store.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectStoreId(u64);

impl ObjectStoreId {
    /// Returns the numeric value of this process-local store identity.
    pub fn value(self) -> u64 {
        self.0
    }
}

/// Identity of one immutable object stored in an `ObjectStore`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(u64);

impl ObjectId {
    /// Returns the numeric value of this store-local object identity.
    pub fn value(self) -> u64 {
        self.0
    }
}

/// In-memory object store that interns identical blocks once per store.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectStore {
    id: ObjectStoreId,
    objects: Vec<Block>,
    index: HashMap<Block, ObjectId>,
}

impl ObjectStore {
    /// Creates an empty in-memory object store.
    pub fn new() -> Self {
        Self {
            id: ObjectStoreId(
                NEXT_OBJECT_STORE_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
            ),
            objects: Vec::new(),
            index: HashMap::new(),
        }
    }

    /// Returns this store's process-local identity.
    pub fn id(&self) -> ObjectStoreId {
        self.id
    }

    /// Returns the number of unique immutable objects in this store.
    pub fn len(&self) -> usize {
        self.objects.len()
    }

    /// Returns whether this store contains no objects.
    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    /// Returns the identity for `block`, inserting it only when its content is new.
    pub fn intern(&mut self, block: Block) -> ObjectId {
        if let Some(id) = self.index.get(&block) {
            return *id;
        }

        let id = ObjectId((self.objects.len() + 1) as u64);
        self.objects.push(block.clone());
        self.index.insert(block, id);
        id
    }

    /// Resolves an object identity to its immutable block contents.
    pub fn get(&self, id: ObjectId) -> Option<&Block> {
        let index = usize::try_from(id.0).ok()?.checked_sub(1)?;
        self.objects.get(index)
    }
}
impl Default for ObjectStore {
    fn default() -> Self {
        Self::new()
    }
}
/// Immutable mapping from logical block positions to shared object identities.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateRoot {
    store_id: ObjectStoreId,
    objects: Vec<ObjectId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StateRootError {
    StoreMismatch {
        root: ObjectStoreId,
        store: ObjectStoreId,
    },
    BlockOutOfRange {
        id: BlockId,
        block_count: usize,
    },
    UnknownObject {
        id: ObjectId,
    },
}

impl StateRoot {
    /// Builds a state root by interning every block into the supplied store.
    pub fn from_blocks(store: &mut ObjectStore, blocks: &[Block]) -> Self {
        let objects = blocks
            .iter()
            .cloned()
            .map(|block| store.intern(block))
            .collect();
        Self {
            store_id: store.id(),
            objects,
        }
    }

    /// Returns the object store identity used by this root.
    pub fn store_id(&self) -> ObjectStoreId {
        self.store_id
    }

    /// Returns the number of logical block positions in this root.
    pub fn block_count(&self) -> usize {
        self.objects.len()
    }

    /// Returns the shared object identity at a logical block position.
    pub fn object_id(&self, id: BlockId) -> Option<ObjectId> {
        self.objects.get(id.index()).copied()
    }

    /// Returns a new root that changes one position while retaining every other reference.
    pub fn replace_object(
        &self,
        store: &ObjectStore,
        id: BlockId,
        object: ObjectId,
    ) -> Result<Self, StateRootError> {
        self.ensure_store(store)?;
        if id.index() >= self.block_count() {
            return Err(StateRootError::BlockOutOfRange {
                id,
                block_count: self.block_count(),
            });
        }
        if store.get(object).is_none() {
            return Err(StateRootError::UnknownObject { id: object });
        }

        let mut objects = self.objects.clone();
        objects[id.index()] = object;
        Ok(Self {
            store_id: self.store_id,
            objects,
        })
    }

    /// Materializes this logical state from the shared object store.
    pub fn materialize(&self, store: &ObjectStore) -> Result<Vec<Block>, StateRootError> {
        self.ensure_store(store)?;
        self.objects
            .iter()
            .map(|id| {
                store
                    .get(*id)
                    .cloned()
                    .ok_or(StateRootError::UnknownObject { id: *id })
            })
            .collect()
    }

    fn ensure_store(&self, store: &ObjectStore) -> Result<(), StateRootError> {
        if self.store_id != store.id() {
            return Err(StateRootError::StoreMismatch {
                root: self.store_id,
                store: store.id(),
            });
        }
        Ok(())
    }
}

/// Process-local identity for one immutable checkpoint snapshot.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct CheckpointId(u64);

impl CheckpointId {
    /// Returns the numeric value of this process-local checkpoint identity.
    pub fn value(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Checkpoint {
    id: CheckpointId,
    block_count: usize,
    overlay: Vec<Option<Block>>,
}

impl Checkpoint {
    /// Returns this snapshot's process-local identity.
    pub fn id(&self) -> CheckpointId {
        self.id
    }
}

/// Relationship between the current overlay and an optional checkpoint.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CheckpointRelation {
    /// No checkpoint comparison was requested.
    NotCompared,
    /// The checkpoint is compatible; `changed_blocks` are overlay differences.
    Compatible {
        checkpoint_id: CheckpointId,
        changed_blocks: Vec<BlockId>,
    },
    /// The checkpoint has a different block-count shape.
    Incompatible {
        checkpoint_id: CheckpointId,
        checkpoint_block_count: usize,
    },
}

/// Deterministic, read-only summary of the engine's current overlay state.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StateInspection {
    /// Total fixed block count.
    pub block_count: usize,
    /// Number of present overlay entries.
    pub dirty_count: usize,
    /// Whether at least one overlay entry is present.
    pub has_changes: bool,
    /// IDs with present overlay entries, in ascending order.
    pub affected_blocks: Vec<BlockId>,
    /// Optional relationship to the requested checkpoint.
    pub checkpoint: CheckpointRelation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChangeSet {
    changes: Vec<(BlockId, Block)>,
}

impl ChangeSet {
    /// Returns the number of pending block changes in this snapshot.
    pub fn len(&self) -> usize {
        self.changes.len()
    }

    /// Returns whether this snapshot contains no pending block changes.
    pub fn is_empty(&self) -> bool {
        self.changes.is_empty()
    }

    /// Returns pending changes in ascending block-ID order.
    pub fn as_slice(&self) -> &[(BlockId, Block)] {
        &self.changes
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TransactionId(u64);

impl TransactionId {
    /// Creates a new opaque process-local transaction identity.
    pub fn new() -> Self {
        Self(NEXT_TRANSACTION_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }

    /// Returns the numeric value of this transaction identity.
    pub fn value(self) -> u64 {
        self.0
    }
}

impl Default for TransactionId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Generation(u64);

impl Generation {
    /// Creates an explicit logical generation value.
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    /// Returns the initial logical generation.
    pub const fn initial() -> Self {
        Self(0)
    }

    /// Returns the numeric generation value.
    pub const fn value(self) -> u64 {
        self.0
    }

    /// Returns the next generation when the counter has not reached its limit.
    pub fn next(self) -> Option<Self> {
        self.0.checked_add(1).map(Self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedState {
    transaction_id: TransactionId,
    base_generation: Generation,
    target_generation: Generation,
    changes: ChangeSet,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PrepareError {
    TargetGenerationNotAhead {
        base: Generation,
        target: Generation,
    },
}

impl PreparedState {
    /// Freezes a change set for one transaction and target logical generation.
    pub fn new(
        transaction_id: TransactionId,
        base_generation: Generation,
        target_generation: Generation,
        changes: ChangeSet,
    ) -> Result<Self, PrepareError> {
        if target_generation <= base_generation {
            return Err(PrepareError::TargetGenerationNotAhead {
                base: base_generation,
                target: target_generation,
            });
        }

        Ok(Self {
            transaction_id,
            base_generation,
            target_generation,
            changes,
        })
    }

    /// Returns the transaction identity associated with this prepared state.
    pub fn transaction_id(&self) -> TransactionId {
        self.transaction_id
    }

    /// Returns the committed generation from which this transaction was prepared.
    pub fn base_generation(&self) -> Generation {
        self.base_generation
    }

    /// Returns the candidate generation published by a successful commit.
    pub fn target_generation(&self) -> Generation {
        self.target_generation
    }

    /// Returns the immutable change set retained for this transaction.
    pub fn changes(&self) -> &ChangeSet {
        &self.changes
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitOutcome {
    /// The target generation is known to be atomically published.
    Committed {
        transaction_id: TransactionId,
        generation: Generation,
    },
    /// The target generation is known not to have been published.
    Aborted { transaction_id: TransactionId },
    /// Publication status cannot be established yet.
    Unknown { transaction_id: TransactionId },
}

impl CommitOutcome {
    /// Returns the transaction identity carried by this outcome.
    pub fn transaction_id(&self) -> TransactionId {
        match self {
            Self::Committed { transaction_id, .. }
            | Self::Aborted { transaction_id }
            | Self::Unknown { transaction_id } => *transaction_id,
        }
    }
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

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RestoreError {
    IncompatibleBlockCount { engine: usize, checkpoint: usize },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DiffError {
    IncompatibleBlockCount { engine: usize, checkpoint: usize },
}

impl Engine {
    /// Creates an in-memory engine with `blocks` zero-filled blocks.
    pub fn new(blocks: usize) -> Self {
        Self {
            base: vec![Block::zeroed(); blocks],
            overlay: vec![None; blocks],
        }
    }

    /// Returns the fixed number of blocks in this engine.
    pub fn block_count(&self) -> usize {
        self.base.len()
    }

    /// Reads a block, preferring its uncommitted overlay value over the base.
    /// Returns `None` when `id` is outside the engine.
    pub fn read(&self, id: BlockId) -> Option<Block> {
        self.overlay
            .get(id.index())
            .and_then(|block| block.as_ref().cloned())
            .or_else(|| self.base.get(id.index()).cloned())
    }

    /// Replaces the overlay value for `id` without changing the base.
    /// An out-of-range ID returns an error and leaves the engine unchanged.
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

    /// Captures an immutable copy of the overlay and its block-count shape.
    pub fn checkpoint(&self) -> Checkpoint {
        Checkpoint {
            id: CheckpointId(NEXT_CHECKPOINT_ID.fetch_add(1, std::sync::atomic::Ordering::Relaxed)),
            block_count: self.block_count(),
            overlay: self.overlay.clone(),
        }
    }

    /// Replaces the overlay with a compatible checkpoint's overlay.
    /// Compatibility is based on block count; base contents are not captured.
    pub fn restore(&mut self, checkpoint: &Checkpoint) -> Result<(), RestoreError> {
        if checkpoint.block_count != self.block_count()
            || checkpoint.overlay.len() != self.block_count()
        {
            return Err(RestoreError::IncompatibleBlockCount {
                engine: self.block_count(),
                checkpoint: checkpoint.block_count,
            });
        }
        self.overlay = checkpoint.overlay.clone();
        Ok(())
    }

    /// Clears all uncommitted overlay values, leaving the base untouched.
    pub fn discard(&mut self) {
        self.overlay.fill(None);
    }

    /// Counts overlay entries, including writes equal to their base value.
    pub fn dirty_count(&self) -> usize {
        self.overlay.iter().filter(|block| block.is_some()).count()
    }

    /// Captures the current overlay as a deterministic, read-only change set.
    /// The snapshot is independent from later writes, restore, discard, or commit.
    pub fn pending_changes(&self) -> ChangeSet {
        let changes = self
            .overlay
            .iter()
            .enumerate()
            .filter_map(|(index, block)| {
                block
                    .as_ref()
                    .map(|block| (BlockId::new(index), block.clone()))
            })
            .collect();
        ChangeSet { changes }
    }

    /// Summarizes the current overlay and optionally compares it to a checkpoint.
    pub fn inspect(&self, checkpoint: Option<&Checkpoint>) -> StateInspection {
        let affected_blocks: Vec<_> = self
            .overlay
            .iter()
            .enumerate()
            .filter_map(|(index, block)| block.as_ref().map(|_| BlockId::new(index)))
            .collect();
        let checkpoint_relation = match checkpoint {
            None => CheckpointRelation::NotCompared,
            Some(saved) => match self.diff(saved) {
                Ok(changed_blocks) => CheckpointRelation::Compatible {
                    checkpoint_id: saved.id(),
                    changed_blocks,
                },
                Err(DiffError::IncompatibleBlockCount { .. }) => CheckpointRelation::Incompatible {
                    checkpoint_id: saved.id(),
                    checkpoint_block_count: saved.block_count,
                },
            },
        };

        StateInspection {
            block_count: self.block_count(),
            dirty_count: affected_blocks.len(),
            has_changes: !affected_blocks.is_empty(),
            affected_blocks,
            checkpoint: checkpoint_relation,
        }
    }

    /// Applies every present overlay value to the in-memory base and clears it.
    /// This operation has no backend and therefore defines no backend failure policy.
    pub fn commit(&mut self) {
        for (base, change) in self.base.iter_mut().zip(self.overlay.iter_mut()) {
            if let Some(block) = change.take() {
                *base = block;
            }
        }
    }

    /// Returns IDs whose overlay entries differ from the checkpoint overlay.
    /// This compares overlay state, not effective block contents or base data.
    pub fn diff(&self, checkpoint: &Checkpoint) -> Result<Vec<BlockId>, DiffError> {
        if checkpoint.block_count != self.block_count()
            || checkpoint.overlay.len() != self.block_count()
        {
            return Err(DiffError::IncompatibleBlockCount {
                engine: self.block_count(),
                checkpoint: checkpoint.block_count,
            });
        }
        Ok(self
            .overlay
            .iter()
            .zip(checkpoint.overlay.iter())
            .enumerate()
            .filter_map(|(index, (current, saved))| {
                (current != saved).then_some(BlockId::new(index))
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::{
        Block, BlockId, ChangeSet, CommitOutcome, DiffError, Engine, Generation, ObjectId,
        ObjectStore, PrepareError, PreparedState, RestoreError, StateRoot, StateRootError,
        TransactionId, WriteError, BLOCK_SIZE,
    };

    fn block(value: u8) -> Block {
        Block::from_bytes([value; BLOCK_SIZE])
    }

    fn id(index: usize) -> BlockId {
        BlockId::new(index)
    }

    #[test]
    fn object_store_interns_identical_blocks_once() {
        let mut store = ObjectStore::new();
        let first = store.intern(block(7));
        let second = store.intern(block(7));
        let third = store.intern(block(8));

        assert_eq!(first, second);
        assert_ne!(first, third);
        assert_eq!(store.len(), 2);
        assert_eq!(store.get(first), Some(&block(7)));
    }

    #[test]
    fn state_roots_share_unchanged_object_references() {
        let mut store = ObjectStore::new();
        let first = StateRoot::from_blocks(&mut store, &[block(1), block(2), block(3)]);
        let replacement = store.intern(block(9));
        let second = first.replace_object(&store, id(1), replacement).unwrap();

        assert_eq!(first.object_id(id(0)), second.object_id(id(0)));
        assert_eq!(first.object_id(id(2)), second.object_id(id(2)));
        assert_ne!(first.object_id(id(1)), second.object_id(id(1)));
        assert_eq!(store.len(), 4);
        assert_eq!(
            first.materialize(&store).unwrap(),
            vec![block(1), block(2), block(3)]
        );
        assert_eq!(
            second.materialize(&store).unwrap(),
            vec![block(1), block(9), block(3)]
        );
    }

    #[test]
    fn unchanged_state_roots_can_be_distinct_without_duplicating_objects() {
        let mut store = ObjectStore::new();
        let first = StateRoot::from_blocks(&mut store, &[block(4), block(5)]);
        let second = StateRoot::from_blocks(&mut store, &[block(4), block(5)]);

        assert_eq!(first, second);
        assert_eq!(first.object_id(id(0)), second.object_id(id(0)));
        assert_eq!(first.object_id(id(1)), second.object_id(id(1)));
        assert_eq!(store.len(), 2);
    }

    #[test]
    fn state_root_rejects_a_different_object_store() {
        let mut first_store = ObjectStore::new();
        let root = StateRoot::from_blocks(&mut first_store, &[block(1)]);
        let mut second_store = ObjectStore::new();
        let object = second_store.intern(block(2));

        assert_eq!(
            root.replace_object(&second_store, id(0), object),
            Err(StateRootError::StoreMismatch {
                root: root.store_id(),
                store: second_store.id(),
            })
        );
    }

    #[test]
    fn state_root_rejects_unknown_object_and_out_of_range_position() {
        let mut store = ObjectStore::new();
        let root = StateRoot::from_blocks(&mut store, &[block(1)]);
        let known = root.object_id(id(0)).unwrap();
        let unknown = ObjectId(known.value() + 10);

        assert_eq!(
            root.replace_object(&store, id(0), unknown),
            Err(StateRootError::UnknownObject { id: unknown })
        );
        assert_eq!(
            root.replace_object(&store, id(1), known),
            Err(StateRootError::BlockOutOfRange {
                id: id(1),
                block_count: 1,
            })
        );
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

        engine.restore(&checkpoint).unwrap();

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
        engine.restore(&checkpoint).unwrap();

        assert_eq!(engine.read(id(2)), Some(block(8)));
    }

    #[test]
    fn diff_reports_overlay_changes() {
        let mut engine = Engine::new(4);
        let checkpoint = engine.checkpoint();

        engine.write(id(0), block(5)).unwrap();
        engine.write(id(3), block(6)).unwrap();

        assert_eq!(engine.diff(&checkpoint), Ok(vec![id(0), id(3)]));
    }

    #[test]
    fn same_as_base_write_is_dirty_and_diff_tracks_overlay_presence() {
        let mut engine = Engine::new(1);
        let checkpoint = engine.checkpoint();

        engine.write(id(0), block(0)).unwrap();

        assert_eq!(engine.read(id(0)), Some(block(0)));
        assert_eq!(engine.block_count(), 1);
        assert_eq!(engine.dirty_count(), 1);
        assert_eq!(engine.diff(&checkpoint), Ok(vec![id(0)]));
    }

    #[test]
    fn incompatible_restore_is_rejected_without_mutation() {
        let mut engine = Engine::new(3);
        engine.write(id(1), block(4)).unwrap();
        let before = engine.clone();
        let checkpoint = Engine::new(2).checkpoint();

        assert_eq!(
            engine.restore(&checkpoint),
            Err(RestoreError::IncompatibleBlockCount {
                engine: 3,
                checkpoint: 2,
            })
        );
        assert_eq!(engine, before);
    }

    #[test]
    fn incompatible_diff_is_explicit() {
        let engine = Engine::new(3);
        let checkpoint = Engine::new(2).checkpoint();

        assert_eq!(
            engine.diff(&checkpoint),
            Err(DiffError::IncompatibleBlockCount {
                engine: 3,
                checkpoint: 2,
            })
        );
    }

    #[test]
    fn compatible_restore_keeps_engine_readable_and_writable() {
        let mut engine = Engine::new(2);
        engine.write(id(0), block(3)).unwrap();
        let checkpoint = engine.checkpoint();
        engine.write(id(0), block(8)).unwrap();

        engine.restore(&checkpoint).unwrap();
        assert_eq!(engine.read(id(0)), Some(block(3)));
        engine.write(id(1), block(9)).unwrap();
        assert_eq!(engine.read(id(1)), Some(block(9)));
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

    #[test]
    fn write_checkpoint_write_restore_restores_the_saved_overlay() {
        let mut engine = Engine::new(3);
        engine.write(id(0), block(1)).unwrap();
        let checkpoint = engine.checkpoint();
        engine.write(id(0), block(2)).unwrap();
        engine.write(id(2), block(3)).unwrap();

        engine.restore(&checkpoint).unwrap();

        assert_eq!(engine.read(id(0)), Some(block(1)));
        assert_eq!(engine.read(id(2)), Some(block(0)));
        assert_eq!(engine.dirty_count(), 1);
    }

    #[test]
    fn max_block_id_is_rejected_without_mutation() {
        let mut engine = Engine::new(2);
        engine.write(id(1), block(7)).unwrap();
        let before = engine.clone();

        assert_eq!(engine.read(BlockId::new(usize::MAX)), None);
        assert_eq!(
            engine.write(BlockId::new(usize::MAX), block(9)),
            Err(WriteError::OutOfRange {
                id: BlockId::new(usize::MAX),
                block_count: 2,
            })
        );
        assert_eq!(engine, before);
    }

    #[test]
    fn invalid_write_preserves_existing_overlay_state() {
        let mut engine = Engine::new(4);
        engine.write(id(2), block(5)).unwrap();
        let before = engine.clone();

        assert!(engine.write(id(4), block(9)).is_err());

        assert_eq!(engine, before);
        assert_eq!(engine.read(id(2)), Some(block(5)));
        assert_eq!(engine.dirty_count(), 1);
    }

    #[test]
    fn dense_overlay_diff_reports_every_modified_block() {
        let mut engine = Engine::new(8);
        let checkpoint = engine.checkpoint();

        for index in 0..engine.block_count() {
            engine.write(id(index), block((index + 1) as u8)).unwrap();
        }

        let expected: Vec<_> = (0..engine.block_count()).map(id).collect();
        assert_eq!(engine.diff(&checkpoint), Ok(expected));
    }

    #[test]
    fn dense_restore_removes_all_changes_after_checkpoint() {
        let mut engine = Engine::new(8);
        engine.write(id(0), block(1)).unwrap();
        engine.write(id(3), block(4)).unwrap();
        let checkpoint = engine.checkpoint();

        for index in 0..engine.block_count() {
            engine.write(id(index), block(9)).unwrap();
        }

        engine.restore(&checkpoint).unwrap();

        assert_eq!(engine.read(id(0)), Some(block(1)));
        assert_eq!(engine.read(id(3)), Some(block(4)));
        for index in [1, 2, 4, 5, 6, 7] {
            assert_eq!(engine.read(id(index)), Some(block(0)));
        }
        assert_eq!(engine.dirty_count(), 2);
    }

    #[test]
    fn committed_state_survives_subsequent_discard_and_restore() {
        let mut engine = Engine::new(3);
        engine.write(id(1), block(6)).unwrap();
        engine.commit();
        let checkpoint = engine.checkpoint();

        engine.write(id(0), block(8)).unwrap();
        engine.discard();
        engine.write(id(2), block(9)).unwrap();
        engine.restore(&checkpoint).unwrap();

        assert_eq!(engine.read(id(1)), Some(block(6)));
        assert_eq!(engine.read(id(0)), Some(block(0)));
        assert_eq!(engine.read(id(2)), Some(block(0)));
        assert_eq!(engine.dirty_count(), 0);
    }

    #[test]
    fn write_checkpoint_write_discard_clears_all_overlay_changes() {
        let mut engine = Engine::new(2);
        engine.write(id(0), block(1)).unwrap();
        let _checkpoint = engine.checkpoint();
        engine.write(id(1), block(2)).unwrap();

        engine.discard();

        assert_eq!(engine.read(id(0)), Some(block(0)));
        assert_eq!(engine.read(id(1)), Some(block(0)));
        assert_eq!(engine.dirty_count(), 0);
    }

    #[test]
    fn commit_checkpoint_write_restore_preserves_committed_base() {
        let mut engine = Engine::new(2);
        engine.write(id(0), block(5)).unwrap();
        engine.commit();
        let checkpoint = engine.checkpoint();
        engine.write(id(0), block(6)).unwrap();

        engine.restore(&checkpoint).unwrap();

        assert_eq!(engine.read(id(0)), Some(block(5)));
        assert_eq!(engine.dirty_count(), 0);
    }

    #[test]
    fn repeated_writes_replace_the_same_overlay_block() {
        let mut engine = Engine::new(1);

        engine.write(id(0), block(1)).unwrap();
        engine.write(id(0), block(2)).unwrap();
        engine.write(id(0), block(3)).unwrap();

        assert_eq!(engine.read(id(0)), Some(block(3)));
        assert_eq!(engine.dirty_count(), 1);
    }

    #[test]
    fn checkpoints_are_immutable_and_an_unchanged_diff_is_empty() {
        let mut engine = Engine::new(2);
        let clean = engine.checkpoint();

        assert_eq!(engine.diff(&clean), Ok(vec![]));
        engine.write(id(1), block(1)).unwrap();
        let changed = engine.checkpoint();
        engine.write(id(1), block(2)).unwrap();

        assert_eq!(engine.diff(&clean), Ok(vec![id(1)]));
        assert_eq!(engine.diff(&changed), Ok(vec![id(1)]));
        assert_eq!(engine.diff(&clean), Ok(vec![id(1)]));
    }

    #[test]
    fn checkpoint_ids_are_distinct_and_restore_the_snapshot_they_identify() {
        let mut engine = Engine::new(1);
        engine.write(id(0), block(1)).unwrap();
        let first = engine.checkpoint();
        engine.write(id(0), block(2)).unwrap();
        let second = engine.checkpoint();

        assert_ne!(first.id(), second.id());
        assert_ne!(first.id().value(), second.id().value());

        engine.write(id(0), block(3)).unwrap();
        engine.restore(&first).unwrap();
        assert_eq!(engine.read(id(0)), Some(block(1)));
        engine.restore(&second).unwrap();
        assert_eq!(engine.read(id(0)), Some(block(2)));
    }

    #[test]
    fn checkpoint_identity_remains_attached_after_commit_and_discard() {
        let mut engine = Engine::new(1);
        engine.write(id(0), block(4)).unwrap();
        let before_commit = engine.checkpoint();
        engine.commit();
        let after_commit = engine.checkpoint();
        assert_ne!(before_commit.id(), after_commit.id());

        engine.write(id(0), block(5)).unwrap();
        engine.discard();
        engine.restore(&before_commit).unwrap();
        assert_eq!(engine.read(id(0)), Some(block(4)));
        engine.write(id(0), block(6)).unwrap();
        engine.restore(&after_commit).unwrap();
        assert_eq!(engine.read(id(0)), Some(block(4)));
        assert_eq!(engine.dirty_count(), 0);
    }

    #[test]
    fn repeated_restore_of_checkpoint_identity_is_stable() {
        let mut engine = Engine::new(1);
        engine.write(id(0), block(7)).unwrap();
        let checkpoint = engine.checkpoint();
        let identity = checkpoint.id();

        engine.write(id(0), block(8)).unwrap();
        engine.restore(&checkpoint).unwrap();
        engine.write(id(0), block(9)).unwrap();
        engine.restore(&checkpoint).unwrap();

        assert_eq!(checkpoint.id(), identity);
        assert_eq!(engine.read(id(0)), Some(block(7)));
        assert_eq!(engine.dirty_count(), 1);
    }

    #[test]
    fn inspection_reports_deterministic_state_and_checkpoint_relation() {
        let mut engine = Engine::new(4);
        let checkpoint = engine.checkpoint();
        engine.write(id(3), block(3)).unwrap();
        engine.write(id(1), block(1)).unwrap();

        let inspection = engine.inspect(Some(&checkpoint));

        assert_eq!(inspection.block_count, 4);
        assert_eq!(inspection.dirty_count, 2);
        assert!(inspection.has_changes);
        assert_eq!(inspection.affected_blocks, vec![id(1), id(3)]);
        assert_eq!(
            inspection.checkpoint,
            super::CheckpointRelation::Compatible {
                checkpoint_id: checkpoint.id(),
                changed_blocks: vec![id(1), id(3)],
            }
        );
        assert_eq!(engine.inspect(Some(&checkpoint)), inspection);
    }

    #[test]
    fn inspection_reports_clean_and_incompatible_states_explicitly() {
        let engine = Engine::new(2);
        let compatible = engine.checkpoint();
        let incompatible = Engine::new(3).checkpoint();

        let clean = engine.inspect(None);
        assert_eq!(clean.block_count, 2);
        assert_eq!(clean.dirty_count, 0);
        assert!(!clean.has_changes);
        assert!(clean.affected_blocks.is_empty());
        assert_eq!(clean.checkpoint, super::CheckpointRelation::NotCompared);
        assert_eq!(
            engine.inspect(Some(&compatible)).checkpoint,
            super::CheckpointRelation::Compatible {
                checkpoint_id: compatible.id(),
                changed_blocks: vec![],
            }
        );
        assert_eq!(
            engine.inspect(Some(&incompatible)).checkpoint,
            super::CheckpointRelation::Incompatible {
                checkpoint_id: incompatible.id(),
                checkpoint_block_count: 3,
            }
        );
    }

    #[test]
    fn pending_change_set_is_sorted_and_contains_current_overlay_values() {
        let mut engine = Engine::new(4);
        engine.write(id(3), block(3)).unwrap();
        engine.write(id(1), block(1)).unwrap();

        let changes = engine.pending_changes();

        assert_eq!(changes.len(), 2);
        assert!(!changes.is_empty());
        assert_eq!(changes.as_slice(), &[(id(1), block(1)), (id(3), block(3))]);
    }

    #[test]
    fn pending_change_set_is_empty_after_new_commit_and_discard() {
        let mut engine = Engine::new(2);
        assert!(engine.pending_changes().is_empty());

        engine.write(id(0), block(4)).unwrap();
        engine.commit();
        assert!(engine.pending_changes().is_empty());

        engine.write(id(1), block(5)).unwrap();
        engine.discard();
        assert!(engine.pending_changes().is_empty());
    }

    #[test]
    fn pending_change_set_is_an_independent_snapshot() {
        let mut engine = Engine::new(2);
        engine.write(id(0), block(7)).unwrap();
        let changes = engine.pending_changes();

        engine.write(id(0), block(8)).unwrap();
        engine.write(id(1), block(9)).unwrap();
        engine.discard();

        assert_eq!(changes.as_slice(), &[(id(0), block(7))]);
    }

    #[test]
    fn checkpoint_after_commit_captures_a_clean_overlay() {
        let mut engine = Engine::new(2);
        engine.write(id(1), block(4)).unwrap();
        engine.commit();
        let checkpoint = engine.checkpoint();
        engine.write(id(0), block(7)).unwrap();

        assert_eq!(engine.diff(&checkpoint), Ok(vec![id(0)]));
        engine.restore(&checkpoint).unwrap();
        assert_eq!(engine.read(id(1)), Some(block(4)));
        assert_eq!(engine.read(id(0)), Some(block(0)));
    }

    #[test]
    fn repeated_commit_discard_and_restore_are_idempotent() {
        let mut engine = Engine::new(1);
        let clean = engine.checkpoint();

        engine.discard();
        engine.commit();
        engine.restore(&clean).unwrap();
        engine.restore(&clean).unwrap();
        assert_eq!(engine.dirty_count(), 0);

        engine.write(id(0), block(8)).unwrap();
        engine.commit();
        engine.commit();
        engine.discard();
        assert_eq!(engine.read(id(0)), Some(block(8)));
        assert_eq!(engine.dirty_count(), 0);
    }

    #[test]
    fn empty_engine_supports_lifecycle_operations() {
        let mut engine = Engine::new(0);
        let checkpoint = engine.checkpoint();

        assert_eq!(engine.block_count(), 0);
        assert_eq!(engine.diff(&checkpoint), Ok(vec![]));
        engine.restore(&checkpoint).unwrap();
        engine.commit();
        engine.discard();
        assert_eq!(engine.dirty_count(), 0);
        assert_eq!(engine.read(id(0)), None);
    }

    #[test]
    fn transaction_ids_are_opaque_and_distinct() {
        let first = TransactionId::new();
        let second = TransactionId::new();

        assert_ne!(first, second);
        assert!(second.value() > first.value());
    }

    #[test]
    fn generations_are_monotonic_until_exhaustion() {
        let initial = Generation::initial();

        assert_eq!(initial.value(), 0);
        assert_eq!(initial.next(), Some(Generation::new(1)));
        assert_eq!(Generation::new(u64::MAX).next(), None);
    }

    #[test]
    fn prepared_state_retains_transaction_identity_generations_and_changes() {
        let transaction_id = TransactionId::new();
        let changes = ChangeSet {
            changes: vec![(id(1), block(7))],
        };

        let prepared = PreparedState::new(
            transaction_id,
            Generation::new(4),
            Generation::new(5),
            changes.clone(),
        )
        .unwrap();

        assert_eq!(prepared.transaction_id(), transaction_id);
        assert_eq!(prepared.base_generation(), Generation::new(4));
        assert_eq!(prepared.target_generation(), Generation::new(5));
        assert_eq!(prepared.changes(), &changes);
    }

    #[test]
    fn prepared_state_rejects_a_target_generation_that_is_not_ahead() {
        let transaction_id = TransactionId::new();
        let changes = ChangeSet {
            changes: vec![(id(0), block(3))],
        };

        assert_eq!(
            PreparedState::new(
                transaction_id,
                Generation::new(5),
                Generation::new(5),
                changes.clone(),
            ),
            Err(PrepareError::TargetGenerationNotAhead {
                base: Generation::new(5),
                target: Generation::new(5),
            })
        );
        assert_eq!(
            PreparedState::new(
                transaction_id,
                Generation::new(6),
                Generation::new(5),
                changes,
            ),
            Err(PrepareError::TargetGenerationNotAhead {
                base: Generation::new(6),
                target: Generation::new(5),
            })
        );
    }

    #[test]
    fn commit_outcomes_keep_the_same_transaction_identity() {
        let transaction_id = TransactionId::new();

        let committed = CommitOutcome::Committed {
            transaction_id,
            generation: Generation::new(9),
        };
        let aborted = CommitOutcome::Aborted { transaction_id };
        let unknown = CommitOutcome::Unknown { transaction_id };

        assert_eq!(committed.transaction_id(), transaction_id);
        assert_eq!(aborted.transaction_id(), transaction_id);
        assert_eq!(unknown.transaction_id(), transaction_id);
    }

}
