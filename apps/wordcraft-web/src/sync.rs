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

/// Status bar text for the hosted document; only a server acknowledgement reads as saved.
pub fn status(loading: bool, saving: bool, failed: bool, dirty: bool, revision: u64) -> &'static str {
    if loading {
        "Opening document…"
    } else if saving {
        "Saving…"
    } else if failed {
        "Not saved"
    } else if dirty {
        "Unsaved changes"
    } else if revision == 0 {
        "New document · save to keep"
    } else {
        "Saved to server"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reports_saved_only_after_a_clean_acknowledged_revision() {
        assert_eq!(status(true, true, true, true, 3), "Opening document…");
        assert_eq!(status(false, true, true, true, 3), "Saving…");
        assert_eq!(status(false, false, true, false, 3), "Not saved");
        assert_eq!(status(false, false, false, true, 3), "Unsaved changes");
        assert_eq!(status(false, false, false, false, 0), "New document · save to keep");
        assert_eq!(status(false, false, false, false, 3), "Saved to server");
    }
    #[test]
    fn delayed_save_does_not_clear_new_edits_or_another_document() {
        let flight = Flight { id: "a".into(), epoch: 1, local_revision: 5 };
        assert!(acknowledges_current(&flight, "a", 1, 5));
        assert!(!acknowledges_current(&flight, "a", 1, 6));
        assert!(!acknowledges_current(&flight, "b", 1, 5));
        assert!(!acknowledges_current(&flight, "a", 2, 5));
    }
}
