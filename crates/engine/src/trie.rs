//! Radix/Prefix Trie for fast exception lookup.

#[derive(Debug, Clone, Default)]
pub struct TrieNode {
    children: Vec<(char, TrieNode)>,
    pub is_word: bool,
}

impl TrieNode {
    fn new() -> Self {
        Self {
            children: Vec::new(),
            is_word: false,
        }
    }

    fn find_child(&self, c: char) -> Option<&TrieNode> {
        self.children
            .iter()
            .find(|(ch, _)| *ch == c)
            .map(|(_, node)| node)
    }

    fn find_child_mut(&mut self, c: char) -> Option<&mut TrieNode> {
        self.children
            .iter_mut()
            .find(|(ch, _)| *ch == c)
            .map(|(_, node)| node)
    }
}

/// A prefix tree (Trie) used to store and quickly check exception words.
#[derive(Debug, Clone)]
pub struct ExceptionTrie {
    root: TrieNode,
}

impl Default for ExceptionTrie {
    fn default() -> Self {
        Self::with_defaults()
    }
}

impl ExceptionTrie {
    /// Create an empty trie.
    pub fn new() -> Self {
        Self {
            root: TrieNode::new(),
        }
    }

    /// Create a trie populated with default Uzbek language exceptions.
    pub fn with_defaults() -> Self {
        let mut trie = Self::new();
        // Common Uzbek words where 's' and 'h' are pronounced separately:
        let defaults = [
            "ishoq",
            "is-hoq",
            "is'hoq",
            "ashob",
            "as-hob",
            "mushaf",
            "mus-haf",
            "ishol",
            "mashad",
        ];

        for word in defaults {
            trie.insert(word);
        }

        trie
    }

    /// Insert a word into the trie (automatically converted to lowercase).
    pub fn insert(&mut self, word: &str) {
        let mut curr = &mut self.root;
        for c in word.chars().flat_map(|ch| ch.to_lowercase()) {
            if curr.find_child(c).is_none() {
                curr.children.push((c, TrieNode::new()));
            }
            curr = curr.find_child_mut(c).expect("child was just added");
        }
        curr.is_word = true;
    }

    /// Check if the given exact word exists in the trie.
    pub fn contains(&self, word: &str) -> bool {
        let mut curr = &self.root;
        for c in word.chars().flat_map(|ch| ch.to_lowercase()) {
            match curr.find_child(c) {
                Some(next) => curr = next,
                None => return false,
            }
        }
        curr.is_word
    }

    /// Check if any word in the trie starts with the given prefix.
    pub fn has_prefix(&self, prefix: &str) -> bool {
        let mut curr = &self.root;
        for c in prefix.chars().flat_map(|ch| ch.to_lowercase()) {
            match curr.find_child(c) {
                Some(next) => curr = next,
                None => return false,
            }
        }
        true
    }

    /// Check if a slice of characters matches an exception prefix.
    pub fn has_prefix_chars(&self, prefix: &[char]) -> bool {
        let mut curr = &self.root;
        for ch in prefix {
            for c in ch.to_lowercase() {
                match curr.find_child(c) {
                    Some(next) => curr = next,
                    None => return false,
                }
            }
        }
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trie_insert_and_lookup() {
        let mut trie = ExceptionTrie::new();
        trie.insert("Ishoq");
        trie.insert("as-hob");

        assert!(trie.contains("ishoq"));
        assert!(trie.contains("ISHOQ"));
        assert!(trie.contains("Ishoq"));
        assert!(trie.contains("as-hob"));
        assert!(!trie.contains("ishonch"));

        assert!(trie.has_prefix("is"));
        assert!(trie.has_prefix("ish"));
        assert!(trie.has_prefix("isho"));
        assert!(!trie.has_prefix("isha"));
    }

    #[test]
    fn test_default_exceptions() {
        let trie = ExceptionTrie::with_defaults();
        assert!(trie.contains("mushaf"));
        assert!(trie.has_prefix("mush"));
        assert!(trie.has_prefix("ash"));
        assert!(!trie.has_prefix("shoh"));
    }
}
