//! Keep server acknowledgement separate from newer edits and document replacement.
#[derive(Clone, Debug, PartialEq)]
pub struct Flight {
    pub id: String,
    pub epoch: u64,
    pub local_revision: u64,
}

/// Only the exact snapshot acknowledged by the server can clear the unsaved indicator.
pub fn acknowledges_current(flight: &Flight, id: &str, epoch: u64, local_revision: u64) -> bool {
    flight.id == id && flight.epoch == epoch && flight.local_revision == local_revision
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delayed_save_does_not_clear_new_edits_or_another_document() {
        let flight = Flight { id: "a".into(), epoch: 1, local_revision: 5 };
        assert!(acknowledges_current(&flight, "a", 1, 5));
        assert!(!acknowledges_current(&flight, "a", 1, 6));
        assert!(!acknowledges_current(&flight, "b", 1, 5));
        assert!(!acknowledges_current(&flight, "a", 2, 5));
    }
}
