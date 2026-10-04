use crate::{
    Block, BlockId, ChangeSet, CommitOutcome, DurabilityObservation, Generation, PreparedState,
    TransactionId,
};

/// Backend-neutral persistence contract for the logical state engine.
///
/// Implementations translate these operations into their physical storage
/// mechanism without redefining publication, recovery, transaction identity,
/// generations, or durability semantics.
pub trait PersistenceBackend {
    type Error;

    /// Returns the number of logical blocks represented by the backend.
    fn block_count(&self) -> usize;

    /// Returns the currently published logical generation.
    fn generation(&self) -> Generation;

    /// Reads a block from the currently published logical state.
    fn read(&self, id: BlockId) -> Result<Option<Block>, Self::Error>;

    /// Persists a prepared change set without publishing its target generation.
    fn prepare(
        &mut self,
        transaction_id: TransactionId,
        changes: &ChangeSet,
    ) -> Result<PreparedState, Self::Error>;

    /// Returns the retained prepared state for a transaction, when present.
    fn prepared(&self, transaction_id: TransactionId) -> Option<PreparedState>;

    /// Attempts logical publication of a prepared generation.
    fn commit(&mut self, prepared: &PreparedState) -> Result<CommitOutcome, Self::Error>;

    /// Reconstructs the observable outcome for a transaction after recovery.
    fn reconcile(&mut self, transaction_id: TransactionId) -> Result<CommitOutcome, Self::Error>;

    /// Explicitly discards a retained prepared transaction.
    fn discard(&mut self, transaction_id: TransactionId) -> Result<(), Self::Error>;

    /// Observes the durability state of an already-confirmed publication.
    fn acknowledge_durability(
        &mut self,
        outcome: &CommitOutcome,
    ) -> Result<DurabilityObservation, Self::Error>;
}

#[cfg(test)]
pub(crate) mod contract_tests {
    use super::PersistenceBackend;
    use crate::{
        Block, BlockId, ChangeSet, CommitOutcome, DurabilityObservation, Engine, Generation,
        PreparedState, TransactionId, BLOCK_SIZE,
    };

    fn block(value: u8) -> Block {
        Block::from_bytes([value; BLOCK_SIZE])
    }

    fn changes(index: usize, value: u8, block_count: usize) -> ChangeSet {
        let mut engine = Engine::new(block_count);
        engine.write(BlockId::new(index), block(value)).unwrap();
        engine.pending_changes()
    }

    /// Executes the backend-neutral behavioral contract against one backend.
    ///
    /// The harness intentionally uses only PersistenceBackend operations and
    /// core semantic types. Backend construction, reopening and physical
    /// recovery remain outside this contract harness.
    pub(crate) fn assert_backend_contract<B>(backend: &mut B)
    where
        B: PersistenceBackend,
        B::Error: std::fmt::Debug,
    {
        assert_eq!(backend.block_count(), 2);
        assert_eq!(backend.generation(), Generation::initial());
        assert_eq!(backend.read(BlockId::new(0)).unwrap(), Some(block(0)));
        assert_eq!(backend.read(BlockId::new(1)).unwrap(), Some(block(0)));

        let tx = TransactionId::new();
        let prepared = backend.prepare(tx, &changes(0, 7, 2)).unwrap();

        assert_eq!(prepared.transaction_id(), tx);
        assert_eq!(prepared.base_generation(), Generation::initial());
        assert_eq!(prepared.target_generation(), Generation::new(1));
        assert_eq!(backend.generation(), Generation::initial());
        assert_eq!(backend.read(BlockId::new(0)).unwrap(), Some(block(0)));

        let retained = backend
            .prepared(tx)
            .expect("prepared state must be retained");
        assert_eq!(retained, prepared);

        assert_eq!(
            backend.reconcile(tx).unwrap(),
            CommitOutcome::Aborted { transaction_id: tx }
        );
        let retry = backend
            .prepared(tx)
            .expect("Aborted must retain prepared state");
        assert_eq!(retry, prepared);

        let committed = backend.commit(&retry).unwrap();
        assert_eq!(
            committed,
            CommitOutcome::Committed {
                transaction_id: tx,
                generation: Generation::new(1),
            }
        );
        assert_eq!(backend.generation(), Generation::new(1));
        assert_eq!(backend.read(BlockId::new(0)).unwrap(), Some(block(7)));
        assert!(backend.prepared(tx).is_none());

        assert_eq!(backend.commit(&prepared).unwrap(), committed);
        assert_eq!(
            backend.reconcile(tx).unwrap(),
            CommitOutcome::Committed {
                transaction_id: tx,
                generation: Generation::new(1),
            }
        );

        let aborted = CommitOutcome::Aborted { transaction_id: tx };
        assert!(backend.acknowledge_durability(&aborted).is_err());

        let unknown = CommitOutcome::Unknown {
            transaction_id: tx,
            generation: Generation::new(2),
        };
        assert!(backend.acknowledge_durability(&unknown).is_err());

        let discard_tx = TransactionId::new();
        backend
            .prepare(discard_tx, &changes(1, 9, 2))
            .expect("second generation can be prepared after publication");
        assert_eq!(backend.generation(), Generation::new(1));
        assert_eq!(backend.read(BlockId::new(1)).unwrap(), Some(block(0)));

        backend.discard(discard_tx).unwrap();
        assert!(backend.prepared(discard_tx).is_none());
        assert_eq!(backend.generation(), Generation::new(1));
        assert_eq!(backend.read(BlockId::new(1)).unwrap(), Some(block(0)));
        assert!(backend.reconcile(discard_tx).is_err());
    }

    /// Executes the backend-neutral recovery contract through a backend-specific
    /// factory that creates fresh instances over the same durable state.
    ///
    /// The factory owns physical configuration; the harness validates only the
    /// semantic state observed after reconstruction.
    pub(crate) fn assert_backend_recovery_contract<B, F>(mut open: F)
    where
        B: PersistenceBackend,
        B::Error: std::fmt::Debug,
        F: FnMut() -> Result<B, B::Error>,
    {
        let committed_tx = TransactionId::new();
        let abandoned_tx = TransactionId::new();

        {
            let mut backend = open().expect("initial backend open must succeed");
            assert_eq!(backend.generation(), Generation::initial());
            assert_eq!(backend.read(BlockId::new(0)).unwrap(), Some(block(0)));

            let prepared = backend
                .prepare(committed_tx, &changes(0, 7, 2))
                .expect("committed transaction must prepare");
            assert_eq!(
                backend.commit(&prepared).unwrap(),
                CommitOutcome::Committed {
                    transaction_id: committed_tx,
                    generation: Generation::new(1),
                }
            );

            backend
                .prepare(abandoned_tx, &changes(1, 9, 2))
                .expect("abandoned transaction must prepare");
        }

        {
            let mut reopened = open().expect("reopened backend must succeed");
            assert_eq!(reopened.generation(), Generation::new(1));
            assert_eq!(reopened.read(BlockId::new(0)).unwrap(), Some(block(7)));
            assert_eq!(reopened.read(BlockId::new(1)).unwrap(), Some(block(0)));

            assert_eq!(
                reopened.reconcile(committed_tx).unwrap(),
                CommitOutcome::Committed {
                    transaction_id: committed_tx,
                    generation: Generation::new(1),
                }
            );
            assert_eq!(
                reopened.reconcile(abandoned_tx).unwrap(),
                CommitOutcome::Aborted {
                    transaction_id: abandoned_tx,
                }
            );

            let retry = reopened
                .prepared(abandoned_tx)
                .expect("Aborted recovery must retain prepared state");
            assert_eq!(
                reopened.commit(&retry).unwrap(),
                CommitOutcome::Committed {
                    transaction_id: abandoned_tx,
                    generation: Generation::new(2),
                }
            );
            assert_eq!(reopened.generation(), Generation::new(2));
            assert_eq!(reopened.read(BlockId::new(1)).unwrap(), Some(block(9)));
        }

        let discarded_tx = TransactionId::new();
        {
            let mut backend = open().expect("second reopened backend must succeed");
            backend
                .prepare(discarded_tx, &changes(0, 11, 2))
                .expect("discard transaction must prepare");
            backend
                .discard(discarded_tx)
                .expect("discard must succeed before reconstruction");
        }

        {
            let mut reopened = open().expect("final reopened backend must succeed");
            assert_eq!(reopened.generation(), Generation::new(2));
            assert_eq!(reopened.read(BlockId::new(0)).unwrap(), Some(block(7)));
            assert_eq!(reopened.read(BlockId::new(1)).unwrap(), Some(block(9)));
            assert!(reopened.reconcile(discarded_tx).is_err());
            assert_eq!(
                reopened.reconcile(abandoned_tx).unwrap(),
                CommitOutcome::Committed {
                    transaction_id: abandoned_tx,
                    generation: Generation::new(2),
                }
            );
        }
    }
    #[derive(Clone, Copy)]
    enum UnknownInjectionMode {
        BeforePublication,
        AfterPublication,
    }

    /// Test-only adapter used to deterministically exercise the established
    /// Unknown outcome without adding a production fault-injection API.
    struct UnknownOnceBackend<B> {
        inner: B,
        mode: UnknownInjectionMode,
        injected: bool,
    }

    impl<B> PersistenceBackend for UnknownOnceBackend<B>
    where
        B: PersistenceBackend,
    {
        type Error = B::Error;

        fn block_count(&self) -> usize {
            self.inner.block_count()
        }

        fn generation(&self) -> Generation {
            self.inner.generation()
        }

        fn read(&self, id: BlockId) -> Result<Option<Block>, Self::Error> {
            self.inner.read(id)
        }

        fn prepare(
            &mut self,
            transaction_id: TransactionId,
            changes: &ChangeSet,
        ) -> Result<PreparedState, Self::Error> {
            self.inner.prepare(transaction_id, changes)
        }

        fn prepared(&self, transaction_id: TransactionId) -> Option<PreparedState> {
            self.inner.prepared(transaction_id)
        }

        fn commit(&mut self, prepared: &PreparedState) -> Result<CommitOutcome, Self::Error> {
            if !self.injected {
                self.injected = true;
                return match self.mode {
                    UnknownInjectionMode::BeforePublication => Ok(CommitOutcome::Unknown {
                        transaction_id: prepared.transaction_id(),
                        generation: prepared.target_generation(),
                    }),
                    UnknownInjectionMode::AfterPublication => match self.inner.commit(prepared)? {
                        CommitOutcome::Committed {
                            transaction_id,
                            generation,
                        } => Ok(CommitOutcome::Unknown {
                            transaction_id,
                            generation,
                        }),
                        other => Ok(other),
                    },
                };
            }

            self.inner.commit(prepared)
        }

        fn reconcile(
            &mut self,
            transaction_id: TransactionId,
        ) -> Result<CommitOutcome, Self::Error> {
            self.inner.reconcile(transaction_id)
        }

        fn discard(&mut self, transaction_id: TransactionId) -> Result<(), Self::Error> {
            self.inner.discard(transaction_id)
        }

        fn acknowledge_durability(
            &mut self,
            outcome: &CommitOutcome,
        ) -> Result<DurabilityObservation, Self::Error> {
            self.inner.acknowledge_durability(outcome)
        }
    }

    /// Verifies both possible resolutions of an uncertain publication:
    /// recovery may establish that publication did not occur, or that it did.
    pub(crate) fn assert_unknown_recovery_contract<B, F>(mut open: F)
    where
        B: PersistenceBackend,
        B::Error: std::fmt::Debug,
        F: FnMut() -> Result<B, B::Error>,
    {
        let aborted_tx = TransactionId::new();
        {
            let backend = open().expect("initial backend open must succeed");
            let mut backend = UnknownOnceBackend {
                inner: backend,
                mode: UnknownInjectionMode::BeforePublication,
                injected: false,
            };

            let prepared = backend
                .prepare(aborted_tx, &changes(0, 7, 2))
                .expect("uncertain transaction must prepare");

            assert_eq!(
                backend.commit(&prepared).unwrap(),
                CommitOutcome::Unknown {
                    transaction_id: aborted_tx,
                    generation: Generation::new(1),
                }
            );
            assert_eq!(
                backend.reconcile(aborted_tx).unwrap(),
                CommitOutcome::Aborted {
                    transaction_id: aborted_tx,
                }
            );

            let retained = backend
                .prepared(aborted_tx)
                .expect("Unknown must retain prepared state until resolution");
            assert_eq!(retained, prepared);

            assert_eq!(
                backend.commit(&retained).unwrap(),
                CommitOutcome::Committed {
                    transaction_id: aborted_tx,
                    generation: Generation::new(1),
                }
            );
        }

        let committed_tx = TransactionId::new();
        {
            let backend = open().expect("second backend open must succeed");
            let mut backend = UnknownOnceBackend {
                inner: backend,
                mode: UnknownInjectionMode::AfterPublication,
                injected: false,
            };

            let prepared = backend
                .prepare(committed_tx, &changes(1, 9, 2))
                .expect("uncertain committed transaction must prepare");

            assert_eq!(
                backend.commit(&prepared).unwrap(),
                CommitOutcome::Unknown {
                    transaction_id: committed_tx,
                    generation: Generation::new(2),
                }
            );
            assert_eq!(
                backend.reconcile(committed_tx).unwrap(),
                CommitOutcome::Committed {
                    transaction_id: committed_tx,
                    generation: Generation::new(1),
                }
            );
            assert!(
                backend.prepared(committed_tx).is_none(),
                "reconciled committed publication must not retain a duplicate prepared transaction"
            );
            assert_eq!(backend.generation(), Generation::new(1));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        contract_tests::{
            assert_backend_contract, assert_backend_recovery_contract,
            assert_unknown_recovery_contract,
        },
        PersistenceBackend,
    };
    use crate::persistence::{FileBackend, PersistenceError};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::{fs, process};

    static NEXT_PATH: AtomicU64 = AtomicU64::new(1);

    fn path() -> PathBuf {
        let n = NEXT_PATH.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("mse-m1-11-{}-{}", process::id(), n))
    }

    #[test]
    fn file_backend_implements_the_backend_neutral_contract() {
        fn assert_backend<B: PersistenceBackend<Error = PersistenceError>>() {}

        assert_backend::<FileBackend>();
    }

    #[test]
    fn file_backend_satisfies_the_behavioral_contract() {
        let file = path();
        let mut backend = FileBackend::open(&file, 2).unwrap();

        assert_backend_contract(&mut backend);

        drop(backend);
        fs::remove_file(file).unwrap();
    }

    #[test]
    fn file_backend_satisfies_the_recovery_contract() {
        let file = path();

        assert_backend_recovery_contract(|| FileBackend::open(&file, 2));

        fs::remove_file(file).unwrap();
    }

    #[test]
    fn file_backend_satisfies_the_unknown_recovery_contract() {
        let file = path();

        assert_unknown_recovery_contract(|| FileBackend::open(&file, 2));

        fs::remove_file(file).unwrap();
    }
}
