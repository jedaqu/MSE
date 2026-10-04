use std::fs::{File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::{
    Block, BlockId, ChangeSet, CommitOutcome, DurabilityObservation, DurabilityObservationError,
    DurabilityState, Generation, PreparedState, TransactionId, BLOCK_SIZE,
};

const MAGIC: &[u8; 8] = b"MSEF0001";
const VERSION: u32 = 1;
const FILE_HEADER: u64 = 20;
const RECORD_HEADER: usize = 33;
const OBJECT: u8 = 1;
const PREPARE: u8 = 2;
const COMMIT: u8 = 3;
const DISCARD: u8 = 4;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PersistenceError {
    Io(io::ErrorKind),
    InvalidHeader,
    UnsupportedVersion(u32),
    Corrupt(u64),
    BlockCountMismatch {
        expected: usize,
        actual: usize,
    },
    BlockOutOfRange {
        id: BlockId,
        block_count: usize,
    },
    GenerationAlreadyPrepared {
        generation: Generation,
    },
    TransactionAlreadyPrepared {
        transaction_id: TransactionId,
    },
    TransactionAlreadyCompleted {
        transaction_id: TransactionId,
    },
    TransactionNotPrepared {
        transaction_id: TransactionId,
    },
    TransactionStateMismatch {
        transaction_id: TransactionId,
    },
    GenerationMismatch {
        expected: Generation,
        actual: Generation,
    },
    GenerationExhausted,
    CommitNotConfirmed {
        transaction_id: TransactionId,
    },
}

impl From<io::Error> for PersistenceError {
    fn from(error: io::Error) -> Self {
        Self::Io(error.kind())
    }
}

#[derive(Clone, Debug)]
struct RetainedPrepare {
    prepared: PreparedState,
    root: Vec<u64>,
}

struct Recovered {
    block_count: usize,
    objects: Vec<Block>,
    root: Vec<u64>,
    generation: Generation,
    prepared: Option<RetainedPrepare>,
    completed: Vec<(TransactionId, Generation)>,
}

pub struct FileBackend {
    path: PathBuf,
    file: File,
    block_count: usize,
    objects: Vec<Block>,
    root: Vec<u64>,
    generation: Generation,
    prepared: Option<RetainedPrepare>,
    completed: Vec<(TransactionId, Generation)>,
}

impl std::fmt::Debug for FileBackend {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("FileBackend")
            .field("path", &self.path)
            .field("block_count", &self.block_count)
            .field("object_count", &self.objects.len())
            .field("generation", &self.generation)
            .field("prepared", &self.prepared.is_some())
            .finish()
    }
}

impl FileBackend {
    /// Opens an append-only persistence file or creates a new one.
    ///
    /// Logical visibility is established by a valid commit marker after a
    /// complete prepare record. Filesystem durability is acknowledged separately.
    pub fn open(path: impl AsRef<Path>, block_count: usize) -> Result<Self, PersistenceError> {
        let path = path.as_ref().to_path_buf();
        let new_file = !path.exists() || path.metadata()?.len() == 0;
        let mut file = OpenOptions::new()
            .create(true)
            .read(true)
            .append(true)
            .open(&path)?;

        if new_file {
            Self::initialize(&mut file, block_count)?;
        }

        let recovered = Self::recover(&mut file, block_count)?;
        Ok(Self {
            path,
            file,
            block_count: recovered.block_count,
            objects: recovered.objects,
            root: recovered.root,
            generation: recovered.generation,
            prepared: recovered.prepared,
            completed: recovered.completed,
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn block_count(&self) -> usize {
        self.block_count
    }

    pub fn generation(&self) -> Generation {
        self.generation
    }

    pub fn object_count(&self) -> usize {
        self.objects.len()
    }

    pub fn read(&self, id: BlockId) -> Result<Option<Block>, PersistenceError> {
        let Some(object_id) = self.root.get(id.index()).copied() else {
            return Ok(None);
        };
        let index = object_id
            .checked_sub(1)
            .and_then(|value| usize::try_from(value).ok())
            .ok_or(PersistenceError::Corrupt(0))?;
        self.objects
            .get(index)
            .cloned()
            .map(Some)
            .ok_or(PersistenceError::Corrupt(0))
    }

    pub fn prepare(
        &mut self,
        transaction_id: TransactionId,
        changes: &ChangeSet,
    ) -> Result<PreparedState, PersistenceError> {
        if self.completed.iter().any(|(id, _)| *id == transaction_id) {
            return Err(PersistenceError::TransactionAlreadyCompleted { transaction_id });
        }
        if let Some(existing) = &self.prepared {
            if existing.prepared.transaction_id() == transaction_id {
                return Err(PersistenceError::TransactionAlreadyPrepared { transaction_id });
            }
            return Err(PersistenceError::GenerationAlreadyPrepared {
                generation: self.generation,
            });
        }

        let target = self
            .generation
            .next()
            .ok_or(PersistenceError::GenerationExhausted)?;

        let mut root = self.root.clone();
        for (id, block) in changes.as_slice() {
            if id.index() >= self.block_count {
                return Err(PersistenceError::BlockOutOfRange {
                    id: *id,
                    block_count: self.block_count,
                });
            }
            let object_id = self.intern(block.clone())?;
            root[id.index()] = object_id;
        }

        let prepared = PreparedState::new(transaction_id, self.generation, target, changes.clone())
            .map_err(|_| PersistenceError::GenerationMismatch {
                expected: target,
                actual: self.generation,
            })?;

        self.append(
            PREPARE,
            transaction_id,
            target,
            &encode_prepare(self.generation, &root),
        )?;
        self.prepared = Some(RetainedPrepare {
            prepared: prepared.clone(),
            root,
        });
        Ok(prepared)
    }

    pub fn prepared(&self, transaction_id: TransactionId) -> Option<&PreparedState> {
        self.prepared
            .as_ref()
            .filter(|entry| entry.prepared.transaction_id() == transaction_id)
            .map(|entry| &entry.prepared)
    }

    pub fn commit(&mut self, prepared: &PreparedState) -> Result<CommitOutcome, PersistenceError> {
        if let Some((_, generation)) = self
            .completed
            .iter()
            .find(|(id, _)| *id == prepared.transaction_id())
        {
            return Ok(CommitOutcome::Committed {
                transaction_id: prepared.transaction_id(),
                generation: *generation,
            });
        }

        let Some(entry) = &self.prepared else {
            return Err(PersistenceError::TransactionNotPrepared {
                transaction_id: prepared.transaction_id(),
            });
        };

        if entry.prepared != *prepared {
            return Err(PersistenceError::TransactionStateMismatch {
                transaction_id: prepared.transaction_id(),
            });
        }
        if entry.prepared.base_generation() != self.generation {
            return Err(PersistenceError::GenerationMismatch {
                expected: self.generation,
                actual: entry.prepared.base_generation(),
            });
        }

        let target = entry.prepared.target_generation();
        if self
            .append(COMMIT, prepared.transaction_id(), target, &[])
            .is_err()
        {
            return Ok(CommitOutcome::Unknown {
                transaction_id: prepared.transaction_id(),
                generation: target,
            });
        }

        let entry = self.prepared.take().expect("prepared entry was checked");
        self.root = entry.root;
        self.generation = target;
        self.completed.push((prepared.transaction_id(), target));

        Ok(CommitOutcome::Committed {
            transaction_id: prepared.transaction_id(),
            generation: target,
        })
    }

    pub fn reconcile(
        &mut self,
        transaction_id: TransactionId,
    ) -> Result<CommitOutcome, PersistenceError> {
        self.reload()?;

        if let Some((_, generation)) = self.completed.iter().find(|(id, _)| *id == transaction_id) {
            return Ok(CommitOutcome::Committed {
                transaction_id,
                generation: *generation,
            });
        }

        if self
            .prepared
            .as_ref()
            .is_some_and(|entry| entry.prepared.transaction_id() == transaction_id)
        {
            return Ok(CommitOutcome::Aborted { transaction_id });
        }

        Err(PersistenceError::TransactionNotPrepared { transaction_id })
    }

    pub fn discard(&mut self, transaction_id: TransactionId) -> Result<(), PersistenceError> {
        if self.completed.iter().any(|(id, _)| *id == transaction_id) {
            return Err(PersistenceError::TransactionAlreadyCompleted { transaction_id });
        }
        let Some(entry) = &self.prepared else {
            return Err(PersistenceError::TransactionNotPrepared { transaction_id });
        };
        if entry.prepared.transaction_id() != transaction_id {
            return Err(PersistenceError::TransactionNotPrepared {
                transaction_id,
            });
        }

        self.append(DISCARD, transaction_id, Generation::initial(), &[])?;
        self.prepared = None;
        Ok(())
    }

    pub fn acknowledge_durability(
        &mut self,
        outcome: &CommitOutcome,
    ) -> Result<DurabilityObservation, PersistenceError> {
        let transaction_id = outcome.transaction_id();
        let generation = match outcome {
            CommitOutcome::Committed { generation, .. } => *generation,
            CommitOutcome::Aborted { .. } | CommitOutcome::Unknown { .. } => {
                return Err(PersistenceError::CommitNotConfirmed { transaction_id })
            }
        };

        let state = match self.file.sync_all() {
            Ok(()) => DurabilityState::Durable,
            Err(_) => DurabilityState::Unknown,
        };

        DurabilityObservation::from_commit(
            &CommitOutcome::Committed {
                transaction_id,
                generation,
            },
            state,
        )
        .map_err(|error| match error {
            DurabilityObservationError::CommitNotConfirmed { transaction_id } => {
                PersistenceError::CommitNotConfirmed { transaction_id }
            }
        })
    }

    fn initialize(file: &mut File, block_count: usize) -> Result<(), PersistenceError> {
        file.set_len(0)?;
        file.seek(SeekFrom::Start(0))?;
        file.write_all(MAGIC)?;
        file.write_all(&VERSION.to_le_bytes())?;
        file.write_all(
            &u64::try_from(block_count)
                .map_err(|_| PersistenceError::InvalidHeader)?
                .to_le_bytes(),
        )?;
        append_record(
            file,
            OBJECT,
            TransactionId::new(),
            Generation::initial(),
            &encode_object(1, &Block::zeroed()),
        )?;
        file.sync_all()?;
        Ok(())
    }

    fn append(
        &mut self,
        kind: u8,
        transaction_id: TransactionId,
        generation: Generation,
        payload: &[u8],
    ) -> Result<(), PersistenceError> {
        append_record(&mut self.file, kind, transaction_id, generation, payload)
    }

    fn intern(&mut self, block: Block) -> Result<u64, PersistenceError> {
        if let Some((index, _)) = self
            .objects
            .iter()
            .enumerate()
            .find(|(_, item)| **item == block)
        {
            return Ok(u64::try_from(index + 1).map_err(|_| PersistenceError::Corrupt(0))?);
        }

        let object_id =
            u64::try_from(self.objects.len() + 1).map_err(|_| PersistenceError::Corrupt(0))?;
        self.append(
            OBJECT,
            TransactionId::new(),
            Generation::initial(),
            &encode_object(object_id, &block),
        )?;
        self.objects.push(block);
        Ok(object_id)
    }

    fn reload(&mut self) -> Result<(), PersistenceError> {
        let mut file = OpenOptions::new()
            .read(true)
            .append(true)
            .open(&self.path)?;
        let recovered = Self::recover(&mut file, self.block_count)?;
        self.file = file;
        self.block_count = recovered.block_count;
        self.objects = recovered.objects;
        self.root = recovered.root;
        self.generation = recovered.generation;
        self.prepared = recovered.prepared;
        self.completed = recovered.completed;
        Ok(())
    }

    fn recover(
        file: &mut File,
        expected_block_count: usize,
    ) -> Result<Recovered, PersistenceError> {
        file.seek(SeekFrom::Start(0))?;
        let mut header = [0u8; FILE_HEADER as usize];
        file.read_exact(&mut header)
            .map_err(|_| PersistenceError::InvalidHeader)?;

        if &header[..8] != MAGIC {
            return Err(PersistenceError::InvalidHeader);
        }
        let version = u32::from_le_bytes(header[8..12].try_into().unwrap());
        if version != VERSION {
            return Err(PersistenceError::UnsupportedVersion(version));
        }
        let block_count = usize::try_from(u64::from_le_bytes(header[12..20].try_into().unwrap()))
            .map_err(|_| PersistenceError::InvalidHeader)?;
        if block_count != expected_block_count {
            return Err(PersistenceError::BlockCountMismatch {
                expected: expected_block_count,
                actual: block_count,
            });
        }

        let mut objects = Vec::new();
        let mut root = Vec::new();
        let mut generation = Generation::initial();
        let mut prepared = None;
        let mut completed = Vec::new();
        let file_len = file.metadata()?.len();
        let mut offset = FILE_HEADER;

        while offset < file_len {
            if file_len - offset < RECORD_HEADER as u64 {
                file.set_len(offset)?;
                break;
            }

            file.seek(SeekFrom::Start(offset))?;
            let mut record = [0u8; RECORD_HEADER];
            file.read_exact(&mut record)?;
            let kind = record[0];
            let tx = TransactionId::from_raw(u64::from_le_bytes(record[1..9].try_into().unwrap()));
            let record_generation =
                Generation::new(u64::from_le_bytes(record[9..17].try_into().unwrap()));
            let payload_len = u64::from_le_bytes(record[17..25].try_into().unwrap());
            let expected_checksum = u64::from_le_bytes(record[25..33].try_into().unwrap());
            let payload_end = offset
                .checked_add(RECORD_HEADER as u64)
                .and_then(|value| value.checked_add(payload_len))
                .ok_or(PersistenceError::Corrupt(offset))?;

            if payload_end > file_len {
                file.set_len(offset)?;
                break;
            }

            let payload_size =
                usize::try_from(payload_len).map_err(|_| PersistenceError::Corrupt(offset))?;
            let mut payload = vec![0u8; payload_size];
            if file.read_exact(&mut payload).is_err()
                || checksum(kind, tx, record_generation, &payload) != expected_checksum
            {
                file.set_len(offset)?;
                break;
            }

            match kind {
                OBJECT => {
                    let (object_id, block) =
                        decode_object(&payload).ok_or(PersistenceError::Corrupt(offset))?;
                    let expected_id = u64::try_from(objects.len() + 1)
                        .map_err(|_| PersistenceError::Corrupt(offset))?;
                    if object_id != expected_id || (objects.is_empty() && block != Block::zeroed()) {
                        return Err(PersistenceError::Corrupt(offset));
                    }
                    objects.push(block);
                    if objects.len() == 1 {
                        root = vec![1; block_count];
                    }
                }
                PREPARE => {
                    if prepared.is_some() {
                        return Err(PersistenceError::Corrupt(offset));
                    }
                    let (base, next_root) = decode_prepare(&payload, block_count)
                        .ok_or(PersistenceError::Corrupt(offset))?;
                    if base != generation || base.next() != Some(record_generation) {
                        return Err(PersistenceError::Corrupt(offset));
                    }
                    if next_root.iter().any(|object_id| {
                        object_id == 0
                            || usize::try_from(*object_id - 1)
                                .map_or(true, |index| index >= objects.len())
                    }) {
                        return Err(PersistenceError::Corrupt(offset));
                    }
                    let changes = root_changes(&root, &next_root, &objects);
                    let prepared_state = PreparedState::new(tx, base, record_generation, changes)
                        .map_err(|_| PersistenceError::Corrupt(offset))?;
                    prepared = Some(RetainedPrepare {
                        prepared: prepared_state,
                        root: next_root,
                    });
                }
                COMMIT => {
                    let Some(entry) = prepared.take() else {
                        return Err(PersistenceError::Corrupt(offset));
                    };
                    if entry.prepared.target_generation() != record_generation
                        || generation.next() != Some(record_generation)
                    {
                        return Err(PersistenceError::Corrupt(offset));
                    }
                    root = entry.root;
                    generation = record_generation;
                    completed.push((tx, record_generation));
                }
                DISCARD => {
                    if record_generation != Generation::initial() || !payload.is_empty() {
                        return Err(PersistenceError::Corrupt(offset));
                    }
                    let Some(entry) = prepared.take() else {
                        return Err(PersistenceError::Corrupt(offset));
                    };
                    if entry.prepared.transaction_id() != tx {
                        return Err(PersistenceError::Corrupt(offset));
                    }
                }
                _ => return Err(PersistenceError::Corrupt(offset)),
            }

            offset = payload_end;
        }

        if objects.first() != Some(&Block::zeroed()) {
            return Err(PersistenceError::Corrupt(offset));
        }
        file.seek(SeekFrom::End(0))?;

        Ok(Recovered {
            block_count,
            objects,
            root,
            generation,
            prepared,
            completed,
        })
    }
}

fn append_record(
    file: &mut File,
    kind: u8,
    transaction_id: TransactionId,
    generation: Generation,
    payload: &[u8],
) -> Result<(), PersistenceError> {
    let len = u64::try_from(payload.len()).map_err(|_| PersistenceError::Corrupt(0))?;
    let sum = checksum(kind, transaction_id, generation, payload);
    let mut header = [0u8; RECORD_HEADER];
    header[0] = kind;
    header[1..9].copy_from_slice(&transaction_id.value().to_le_bytes());
    header[9..17].copy_from_slice(&generation.value().to_le_bytes());
    header[17..25].copy_from_slice(&len.to_le_bytes());
    header[25..33].copy_from_slice(&sum.to_le_bytes());
    file.seek(SeekFrom::End(0))?;
    file.write_all(&header)?;
    file.write_all(payload)?;
    Ok(())
}

fn encode_object(id: u64, block: &Block) -> Vec<u8> {
    let mut output = Vec::with_capacity(8 + BLOCK_SIZE);
    output.extend_from_slice(&id.to_le_bytes());
    output.extend_from_slice(block.as_bytes());
    output
}

fn decode_object(payload: &[u8]) -> Option<(u64, Block)> {
    if payload.len() != 8 + BLOCK_SIZE {
        return None;
    }
    Some((
        u64::from_le_bytes(payload[..8].try_into().ok()?),
        Block::from_bytes(payload[8..].try_into().ok()?),
    ))
}

fn encode_prepare(base: Generation, root: &[u64]) -> Vec<u8> {
    let mut output = Vec::with_capacity(16 + root.len() * 8);
    output.extend_from_slice(&base.value().to_le_bytes());
    output.extend_from_slice(&(root.len() as u64).to_le_bytes());
    for object_id in root {
        output.extend_from_slice(&object_id.to_le_bytes());
    }
    output
}

fn decode_prepare(payload: &[u8], block_count: usize) -> Option<(Generation, Vec<u64>)> {
    if payload.len() < 16 {
        return None;
    }
    let base = Generation::new(u64::from_le_bytes(payload[..8].try_into().ok()?));
    let count = usize::try_from(u64::from_le_bytes(payload[8..16].try_into().ok()?)).ok()?;
    if count != block_count || payload.len() != 16 + count.checked_mul(8)? {
        return None;
    }
    Some((
        base,
        payload[16..]
            .chunks_exact(8)
            .map(|chunk| u64::from_le_bytes(chunk.try_into().unwrap()))
            .collect(),
    ))
}

fn root_changes(current: &[u64], next: &[u64], objects: &[Block]) -> ChangeSet {
    let changes = current
        .iter()
        .zip(next.iter())
        .enumerate()
        .filter_map(|(index, (old, new))| {
            if old == new {
                return None;
            }
            let object_index = usize::try_from(*new).ok()?.checked_sub(1)?;
            Some((BlockId::new(index), objects.get(object_index)?.clone()))
        })
        .collect();
    ChangeSet { changes }
}

fn checksum(
    kind: u8,
    transaction_id: TransactionId,
    generation: Generation,
    payload: &[u8],
) -> u64 {
    let mut hash = 0xcbf29ce484222325u64;
    for byte in [kind]
        .into_iter()
        .chain(transaction_id.value().to_le_bytes())
        .chain(generation.value().to_le_bytes())
        .chain(payload.iter().copied())
    {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

#[cfg(test)]
mod tests {
    use super::{FileBackend, PersistenceError};
    use crate::{
        Block, BlockId, CommitOutcome, DurabilityState, Engine, Generation, TransactionId,
        BLOCK_SIZE,
    };
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_PATH: AtomicU64 = AtomicU64::new(1);

    fn block(value: u8) -> Block {
        Block::from_bytes([value; BLOCK_SIZE])
    }

    fn path() -> PathBuf {
        let n = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("mse-m1-9-{}-{}", std::process::id(), n))
    }

    fn changes(index: usize, value: u8, blocks: usize) -> crate::ChangeSet {
        let mut engine = Engine::new(blocks);
        engine.write(BlockId::new(index), block(value)).unwrap();
        engine.pending_changes()
    }

    #[test]
    fn genesis_is_zero_and_generation_is_initial() {
        let file = path();
        let backend = FileBackend::open(&file, 2).unwrap();
        assert_eq!(backend.generation(), Generation::initial());
        assert_eq!(backend.read(BlockId::new(0)).unwrap(), Some(block(0)));
        assert_eq!(backend.read(BlockId::new(1)).unwrap(), Some(block(0)));
        assert_eq!(backend.object_count(), 1);
        drop(backend);
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn prepare_is_not_visible_and_new_blocks_are_deduplicated() {
        let file = path();
        let mut backend = FileBackend::open(&file, 2).unwrap();
        let tx = TransactionId::new();
        let prepared = backend.prepare(tx, &changes(0, 7, 2)).unwrap();
        assert_eq!(prepared.target_generation(), Generation::new(1));
        assert_eq!(backend.generation(), Generation::initial());
        assert_eq!(backend.read(BlockId::new(0)).unwrap(), Some(block(0)));
        assert_eq!(backend.object_count(), 2);
        assert_eq!(
            backend.prepare(TransactionId::new(), &changes(1, 8, 2)),
            Err(PersistenceError::GenerationAlreadyPrepared {
                generation: Generation::initial()
            })
        );
        backend.discard(tx).unwrap();
        drop(backend);
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn commit_publishes_then_durability_is_separate() {
        let file = path();
        let mut backend = FileBackend::open(&file, 2).unwrap();
        let tx = TransactionId::new();
        let prepared = backend.prepare(tx, &changes(1, 9, 2)).unwrap();

        let outcome = backend.commit(&prepared).unwrap();
        assert_eq!(
            outcome,
            CommitOutcome::Committed {
                transaction_id: tx,
                generation: Generation::new(1)
            }
        );
        assert_eq!(backend.read(BlockId::new(1)).unwrap(), Some(block(9)));
        assert_eq!(
            backend.acknowledge_durability(&outcome).unwrap().state(),
            DurabilityState::Durable
        );

        drop(backend);
        let reopened = FileBackend::open(&file, 2).unwrap();
        assert_eq!(reopened.generation(), Generation::new(1));
        assert_eq!(reopened.read(BlockId::new(1)).unwrap(), Some(block(9)));
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn uncommitted_prepare_is_aborted_by_recovery_and_can_retry_same_transaction() {
        let file = path();
        let mut backend = FileBackend::open(&file, 2).unwrap();
        let tx = TransactionId::new();
        backend.prepare(tx, &changes(0, 6, 2)).unwrap();
        drop(backend);

        let mut reopened = FileBackend::open(&file, 2).unwrap();
        assert_eq!(reopened.generation(), Generation::initial());
        assert_eq!(
            reopened.reconcile(tx).unwrap(),
            CommitOutcome::Aborted { transaction_id: tx }
        );
        let retry = reopened.prepared(tx).unwrap().clone();
        assert!(matches!(
            reopened.commit(&retry).unwrap(),
            CommitOutcome::Committed { .. }
        ));

        drop(reopened);
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn discard_survives_reopen() {
        let file = path();
        let mut backend = FileBackend::open(&file, 2).unwrap();
        let tx = TransactionId::new();
        backend.prepare(tx, &changes(1, 4, 2)).unwrap();
        backend.discard(tx).unwrap();
        drop(backend);

        let mut reopened = FileBackend::open(&file, 2).unwrap();
        assert_eq!(
            reopened.reconcile(tx),
            Err(PersistenceError::TransactionNotPrepared { transaction_id: tx })
        );
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn unchanged_objects_are_shared_between_generations() {
        let file = path();
        let mut backend = FileBackend::open(&file, 3).unwrap();

        let tx1 = TransactionId::new();
        let p1 = backend.prepare(tx1, &changes(1, 7, 3)).unwrap();
        backend.commit(&p1).unwrap();
        let objects_after_first = backend.object_count();

        let tx2 = TransactionId::new();
        let p2 = backend.prepare(tx2, &changes(2, 8, 3)).unwrap();
        backend.commit(&p2).unwrap();

        assert_eq!(backend.object_count(), objects_after_first + 1);
        assert_eq!(backend.read(BlockId::new(1)).unwrap(), Some(block(7)));
        assert_eq!(backend.read(BlockId::new(2)).unwrap(), Some(block(8)));

        drop(backend);
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn incomplete_tail_is_not_treated_as_publication() {
        let file = path();
        let mut backend = FileBackend::open(&file, 2).unwrap();
        let tx = TransactionId::new();
        backend.prepare(tx, &changes(0, 5, 2)).unwrap();
        drop(backend);

        let mut raw = OpenOptions::new().append(true).open(&file).unwrap();
        raw.write_all(&[3, 0, 0, 0, 0]).unwrap();
        drop(raw);

        let mut reopened = FileBackend::open(&file, 2).unwrap();
        assert_eq!(reopened.generation(), Generation::initial());
        assert_eq!(
            reopened.reconcile(tx).unwrap(),
            CommitOutcome::Aborted { transaction_id: tx }
        );
        fs::remove_file(file).unwrap();
    }
}
