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
pub struct LocalAgreement {
    prev: Option<String>,
    committed_chars: usize,
}

impl LocalAgreement {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn feed(&mut self, hypothesis: &str) -> Agreement {
        let agreed_chars = match &self.prev {
            Some(prev) => common_prefix_chars(prev, hypothesis),
            None => 0,
        };
        let commit_to = agreed_chars.max(self.committed_chars);
        let chars: Vec<char> = hypothesis.chars().collect();
        let delta_from = self.committed_chars.min(chars.len());
        let delta_to = commit_to.min(chars.len());
        let committed_delta: String = chars[delta_from..delta_to].iter().collect();
        let volatile: String = chars[delta_to..].iter().collect();
        self.committed_chars = commit_to;
        self.prev = Some(hypothesis.to_string());
        Agreement {
            committed_delta,
            volatile,
        }
    }
}

fn common_prefix_chars(a: &str, b: &str) -> usize {
    a.chars()
        .zip(b.chars())
        .take_while(|(x, y)| x == y)
        .count()
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

    #[test]
    fn shorter_hypothesis_than_committed_yields_nothing() {
        let mut la = LocalAgreement::new();
        la.feed("こんにちは、世界");
        la.feed("こんにちは、世界"); // fully committed
        let out = la.feed("こんに"); // hypothesis shrank below the committed point
        assert_eq!(out.committed_delta, "");
        assert_eq!(out.volatile, "");
    }

    #[test]
    fn agreeing_prefix_of_two_hypotheses_is_committed() {
        let mut la = LocalAgreement::new();
        la.feed("こんにちは、せ");
        let out = la.feed("こんにちは、世界");
        assert_eq!(out.committed_delta, "こんにちは、");
        assert_eq!(out.volatile, "世界");
    }
}
