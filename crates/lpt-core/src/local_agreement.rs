//! LocalAgreement-2: a hypothesis prefix is committed once two consecutive
//! hypotheses agree on it. Committed text never retracts.
//!
//! Char-level agreement so it works for Japanese (no word boundaries) and
//! English alike.

/// Result of feeding one hypothesis.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Agreement {
    /// Newly committed text (delta since the previous feed).
    pub committed_delta: String,
    /// Unstable tail of the current hypothesis (may change next feed).
    pub volatile: String,
}

#[derive(Debug, Default)]
pub struct LocalAgreement;

impl LocalAgreement {
    pub fn new() -> Self {
        Self
    }

    pub fn feed(&mut self, hypothesis: &str) -> Agreement {
        Agreement {
            committed_delta: String::new(),
            volatile: hypothesis.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_hypothesis_commits_nothing() {
        let mut la = LocalAgreement::new();
        let out = la.feed("こんにちは");
        assert_eq!(out.committed_delta, "");
        assert_eq!(out.volatile, "こんにちは");
    }
}
