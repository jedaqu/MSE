use crate::{
    Block, BlockId, ChangeSet, CommitOutcome, DurabilityObservation, Generation, PreparedState,
    TransactionId,
};

/// Backend-neutral persistence contract for the logical state engine.
///
/// Implementations translate these operations into their physical storage
/// mechanism without redefining the semantics of publication, recovery,
/// transaction identity, generations, or durability.
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
mod tests {
    use super::PersistenceBackend;
    use crate::persistence::{FileBackend, PersistenceError};

    fn assert_backend<B: PersistenceBackend<Error = PersistenceError>>() {}

    #[test]
    fn file_backend_implements_the_backend_neutral_contract() {
        assert_backend::<FileBackend>();
    }
}
