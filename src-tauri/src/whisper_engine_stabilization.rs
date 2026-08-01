pub fn stable_common_prefix(left: &str, right: &str) -> Option<String> {
    let mut end = 0;
    for ((left_index, left_char), (_, right_char)) in left.char_indices().zip(right.char_indices()) {
        if left_char != right_char {
            break;
        }
        end = left_index + left_char.len_utf8();
    }

    if end == 0 {
        return None;
    }

    let prefix = &left[..end];
    if left == right {
        return Some(prefix.to_string());
    }

    let boundary = prefix
        .char_indices()
        .filter_map(|(index, character)| character.is_whitespace().then_some(index + character.len_utf8()))
        .last()?;
    (boundary > 0).then(|| prefix[..boundary].to_string())
}

#[derive(Debug, Default)]
pub struct PartialStabilizer {
    previous: Option<String>,
    committed_prefix: String,
}

impl PartialStabilizer {
    pub fn observe(&mut self, text: &str) -> Option<String> {
        let previous = self.previous.replace(text.to_string())?;
        let stable = stable_common_prefix(&previous, text)?;
        if stable.len() <= self.committed_prefix.len() {
            return None;
        }

        self.committed_prefix = stable.clone();
        Some(stable)
    }

    pub fn reset(&mut self) {
        self.previous = None;
        self.committed_prefix.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn returns_word_boundary_common_prefix_for_two_partial_results() {
        assert_eq!(
            stable_common_prefix("hello wor", "hello world"),
            Some("hello ".to_string())
        );
    }

    #[test]
    fn stabilizer_waits_for_two_results_and_suppresses_duplicates() {
        let mut stabilizer = PartialStabilizer::default();

        assert_eq!(stabilizer.observe("hello wor"), None);
        assert_eq!(stabilizer.observe("hello world"), Some("hello ".to_string()));
        assert_eq!(stabilizer.observe("hello world!"), None);
    }
}
