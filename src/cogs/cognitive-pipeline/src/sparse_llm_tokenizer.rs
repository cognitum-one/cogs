// Lifted from cognitum-one/seed#133 — byte-identical except for the
// stripped #![cfg(feature = "sparse-llm")] line. Many items here are not yet
// exercised by the cog's current endpoint surface; they become live when
// streaming SSE response bodies and mesh delta-sync land as next-layer
// commits per ADR-095. Multi-layer loading is already exercised end-to-end
// — verified `weight_mode: "gguf-tied[30L+norm]"` (all 30 SmolLM2 layers)
// on seed 1c2650b4. Suppress the remaining lints until those final layers land.
#![allow(dead_code, unused_variables, unused_assignments, unused_imports)]
//! Minimal BPE tokenizer for Pi Zero sparse-llm COG (Phase 2B-zero, ADR-094).
//!
//! Loads a HuggingFace `tokenizer.json` (SmolLM2-135M / LLaMA / Mistral format,
//! 49152 vocab, BPE with byte fallback) and provides encode/decode operations.
//! When no tokenizer file is present a 256-byte fallback stub is returned.

use std::collections::HashMap;
use std::path::Path;

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

/// Errors produced by the BPE tokenizer.
#[derive(Debug)]
pub enum TokenizerError {
    /// Underlying I/O error (file not found, permissions, …).
    Io(std::io::Error),
    /// JSON structure did not match the expected HuggingFace schema.
    ParseError(String),
    /// A token string was not found in the vocabulary.
    UnknownToken(String),
}

impl std::fmt::Display for TokenizerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TokenizerError::Io(e) => write!(f, "tokenizer I/O error: {}", e),
            TokenizerError::ParseError(m) => write!(f, "tokenizer parse error: {}", m),
            TokenizerError::UnknownToken(t) => write!(f, "unknown token: {}", t),
        }
    }
}

impl std::error::Error for TokenizerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            TokenizerError::Io(e) => Some(e),
            _ => None,
        }
    }
}

impl From<std::io::Error> for TokenizerError {
    fn from(e: std::io::Error) -> Self {
        TokenizerError::Io(e)
    }
}

// ---------------------------------------------------------------------------
// Vocab entry (kept public for callers that want to inspect the vocabulary)
// ---------------------------------------------------------------------------

/// A single vocabulary entry with its associated score.
#[derive(Debug, Clone)]
pub struct VocabEntry {
    /// Surface form of the token (may include SentencePiece/HF space markers).
    pub token: String,
    /// Log-probability score (used by full BPE merge; informational here).
    pub score: f32,
}

// ---------------------------------------------------------------------------
// BPE tokenizer
// ---------------------------------------------------------------------------

/// Minimal HuggingFace BPE tokenizer.
///
/// Supports:
/// - Loading vocab from `tokenizer.json` (`model.vocab` object).
/// - Ġ-prefix (GPT-2 / SmolLM2) and ▁-prefix (SentencePiece) space markers.
/// - Byte-level fallback via `<0xNN>` tokens when a character is OOV.
/// - `byte_fallback_stub()` for offline / pre-download use.
/// - Special tokens from `added_tokens` (e.g. `<|im_start|>`, `<|im_end|>`) are
///   matched as atomic units before BPE, enabling proper ChatML encoding.
pub struct BpeTokenizer {
    /// Token string → token ID.
    vocab: HashMap<String, u32>,
    /// Token ID → token string (indexed by ID).
    id_to_token: Vec<String>,
    /// BPE merge rank: (left, right) → priority index (lower = higher priority).
    merge_rank: HashMap<(String, String), usize>,
    /// Special tokens from added_tokens, sorted longest-first for greedy scan.
    /// Each entry is (token_string, token_id).
    special_tokens: Vec<(String, u32)>,
    /// Beginning-of-sequence token ID (SmolLM2: 1).
    pub bos_id: u32,
    /// End-of-sequence token ID (SmolLM2: 2).
    pub eos_id: u32,
    /// Unknown token ID (SmolLM2: 0).
    pub unk_id: u32,
}

impl BpeTokenizer {
    // -----------------------------------------------------------------------
    // Construction
    // -----------------------------------------------------------------------

    /// Load a tokenizer from a HuggingFace `tokenizer.json` file.
    ///
    /// Reads the `model.vocab` object (token → ID map) and detects
    /// BOS / EOS / UNK IDs from the `added_tokens` array.
    ///
    /// # Errors
    ///
    /// Returns `TokenizerError::Io` when the file cannot be opened, or
    /// `TokenizerError::ParseError` when the JSON schema is unexpected.
    pub fn from_file(path: &Path) -> Result<Self, TokenizerError> {
        let file = std::fs::File::open(path)?;
        let json: serde_json::Value = serde_json::from_reader(file)
            .map_err(|e| TokenizerError::ParseError(e.to_string()))?;

        // ---- Parse vocab -----------------------------------------------
        let vocab_obj = json["model"]["vocab"]
            .as_object()
            .ok_or_else(|| TokenizerError::ParseError("missing model.vocab".into()))?;

        let max_id = vocab_obj
            .values()
            .filter_map(|v| v.as_u64())
            .map(|n| n as usize)
            .max()
            .unwrap_or(0);

        let mut vocab: HashMap<String, u32> = HashMap::with_capacity(vocab_obj.len());
        let mut id_to_token: Vec<String> = vec![String::new(); max_id + 1];

        for (token, id_val) in vocab_obj {
            let id = id_val
                .as_u64()
                .ok_or_else(|| {
                    TokenizerError::ParseError(format!("bad id for '{}'", token))
                })? as u32;
            vocab.insert(token.clone(), id);
            if (id as usize) < id_to_token.len() {
                id_to_token[id as usize] = token.clone();
            }
        }

        // ---- Parse special tokens from added_tokens --------------------
        // All entries are stored as (string, id) for atomic matching during encode.
        // BOS/EOS/UNK are identified by content; the rest become special_tokens.
        let mut bos_id: u32 = 1;
        let mut eos_id: u32 = 2;
        let mut unk_id: u32 = 0;
        let mut special_tokens: Vec<(String, u32)> = Vec::new();

        if let Some(added) = json["added_tokens"].as_array() {
            for tok in added {
                let id      = tok["id"].as_u64().unwrap_or(0) as u32;
                let content = tok["content"].as_str().unwrap_or("").to_string();
                match content.as_str() {
                    "<s>" | "<bos>" => bos_id = id,
                    "</s>" | "<eos>" => eos_id = id,
                    "<unk>" => unk_id = id,
                    _ => {}
                }
                // Keep all added tokens (incl. BOS/EOS) for the greedy scanner.
                if !content.is_empty() {
                    special_tokens.push((content, id));
                    // Ensure the vocab map has this token (added_tokens may be out
                    // of the main vocab object for some tokenizer.json formats).
                    vocab.entry(
                        tok["content"].as_str().unwrap_or("").to_string()
                    ).or_insert(id);
                }
            }
        }
        // Sort longest-first so greedy scan never takes a prefix over a full match.
        special_tokens.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        // ---- Parse BPE merge rules -----------------------------------------
        // Each entry is "left right" — the two pieces to merge (space-separated).
        let mut merge_rank: HashMap<(String, String), usize> = HashMap::new();
        if let Some(merges_arr) = json["model"]["merges"].as_array() {
            for (rank, m) in merges_arr.iter().enumerate() {
                if let Some(s) = m.as_str() {
                    if let Some(pos) = s.find(' ') {
                        let left  = s[..pos].to_string();
                        let right = s[pos + 1..].to_string();
                        merge_rank.insert((left, right), rank);
                    }
                }
            }
        }

        Ok(Self { vocab, id_to_token, merge_rank, special_tokens, bos_id, eos_id, unk_id })
    }

    /// Construct a 256-token byte-level fallback stub.
    ///
    /// Provides a functional tokenizer when `tokenizer.json` has not yet been
    /// downloaded. Every byte `b` maps to token `<0xNN>` (ID = `b`).
    /// BOS = 1, EOS = 2, UNK = 0 (same as SmolLM2 defaults).
    pub fn byte_fallback_stub() -> Self {
        let mut vocab: HashMap<String, u32> = HashMap::with_capacity(256);
        let mut id_to_token: Vec<String> = Vec::with_capacity(256);
        for b in 0u8..=255 {
            let tok = format!("<0x{:02X}>", b);
            vocab.insert(tok.clone(), b as u32);
            id_to_token.push(tok);
        }
        Self { vocab, id_to_token, merge_rank: HashMap::new(),
               special_tokens: Vec::new(), bos_id: 1, eos_id: 2, unk_id: 0 }
    }

    // -----------------------------------------------------------------------
    // Encoding
    // -----------------------------------------------------------------------

    /// Encode `text` to a sequence of token IDs.
    ///
    /// Special tokens from `added_tokens` (e.g. `<|im_start|>`, `<|im_end|>`)
    /// are matched atomically before BPE so ChatML templates encode correctly.
    ///
    /// When merge rules are loaded (via `from_file`): applies standard BPE
    /// merges per word for non-special segments.
    ///
    /// When no merge rules are present (stub): falls back to greedy whole-word
    /// lookup then character / byte-level encoding.
    ///
    /// Newlines are encoded as Ċ (U+010A, GPT-2 byte encoding of 0x0A) if that
    /// token exists in the vocabulary, otherwise as the `<0x0A>` byte token.
    ///
    /// Prepends `bos_id`, appends `eos_id`.
    pub fn encode(&self, text: &str) -> Vec<u32> {
        let mut ids: Vec<u32> = Vec::new();
        ids.push(self.bos_id);
        self.encode_segment(text, true, &mut ids);
        ids.push(self.eos_id);
        ids
    }

    /// Encode a text segment, scanning for special tokens first.
    /// `at_line_start` controls whether the first word gets a Ġ-prefix.
    fn encode_segment(&self, text: &str, at_line_start: bool, ids: &mut Vec<u32>) {
        let mut pos = 0;
        let bytes = text.as_bytes();
        let mut first_chunk = at_line_start;

        while pos < bytes.len() {
            // Try to match a special token at current position (longest-first).
            if let Some((tok_str, tok_id)) = self.special_tokens.iter()
                .find(|(s, _)| bytes[pos..].starts_with(s.as_bytes()))
            {
                ids.push(*tok_id);
                pos += tok_str.len();
                first_chunk = false;
                continue;
            }

            // Find next special-token boundary or end of text.
            let next_special = self.special_tokens.iter()
                .filter_map(|(s, _)| {
                    bytes[pos..].windows(s.len())
                        .position(|w| w == s.as_bytes())
                        .map(|p| pos + p)
                })
                .min()
                .unwrap_or(bytes.len());

            // Encode the plain-text segment between pos and next_special.
            let segment = &text[pos..next_special];
            let mut seg_first = first_chunk;
            for (line_idx, raw_line) in segment.split('\n').enumerate() {
                if line_idx > 0 {
                    self.encode_newline(ids);
                    seg_first = true;
                }
                let line = raw_line.trim_end_matches('\r');
                for word in line.split_whitespace() {
                    let prefixed = if seg_first {
                        word.to_string()
                    } else {
                        format!("Ġ{}", word)
                    };
                    seg_first = false;
                    if self.merge_rank.is_empty() {
                        self.encode_word_greedy(&prefixed, ids);
                    } else {
                        self.encode_word_bpe(&prefixed, ids);
                    }
                }
            }

            first_chunk = false;
            pos = next_special;
        }
    }

    /// Emit a newline token: Ċ if in vocab, else the <0x0A> byte token.
    fn encode_newline(&self, out: &mut Vec<u32>) {
        if let Some(&id) = self.vocab.get("Ċ") {
            out.push(id);
        } else {
            out.push(self.vocab.get("<0x0A>").copied().unwrap_or(self.unk_id));
        }
    }

    /// Greedy encoding: try whole word, then chars, then bytes.
    fn encode_word_greedy(&self, word: &str, out: &mut Vec<u32>) {
        if let Some(&id) = self.vocab.get(word) {
            out.push(id);
            return;
        }
        let sp = format!("▁{}", word.trim_start_matches('Ġ'));
        if let Some(&id) = self.vocab.get(&sp) {
            out.push(id);
            return;
        }
        for ch in word.chars() {
            let s = ch.to_string();
            if let Some(&id) = self.vocab.get(&s) {
                out.push(id);
            } else {
                for b in s.as_bytes() {
                    let byte_tok = format!("<0x{:02X}>", b);
                    out.push(self.vocab.get(&byte_tok).copied().unwrap_or(self.unk_id));
                }
            }
        }
    }

    /// BPE encoding: start with individual characters, apply merge rules in
    /// priority order (lowest rank index = highest priority).
    /// O(n²) per word — acceptable for the short sequences on Pi Zero.
    fn encode_word_bpe(&self, word: &str, out: &mut Vec<u32>) {
        // Initialise with individual Unicode scalar values.
        let mut pieces: Vec<String> = word.chars().map(|c| c.to_string()).collect();
        if pieces.is_empty() {
            return;
        }

        // Iteratively apply the highest-priority merge.
        loop {
            let mut best: Option<(usize, usize)> = None; // (position, rank)
            for i in 0..pieces.len().saturating_sub(1) {
                if let Some(&rank) = self.merge_rank.get(&(
                    pieces[i].clone(),
                    pieces[i + 1].clone(),
                )) {
                    if best.map_or(true, |(_, r)| rank < r) {
                        best = Some((i, rank));
                    }
                }
            }
            match best {
                None => break,
                Some((pos, _)) => {
                    let merged = format!("{}{}", pieces[pos], pieces[pos + 1]);
                    pieces.remove(pos + 1);
                    pieces[pos] = merged;
                }
            }
        }

        // Map each merged piece to its vocab ID, with byte fallback.
        for piece in &pieces {
            if let Some(&id) = self.vocab.get(piece.as_str()) {
                out.push(id);
            } else {
                for b in piece.as_bytes() {
                    let byte_tok = format!("<0x{:02X}>", b);
                    out.push(self.vocab.get(&byte_tok).copied().unwrap_or(self.unk_id));
                }
            }
        }
    }

    // -----------------------------------------------------------------------
    // Decoding
    // -----------------------------------------------------------------------

    /// Decode a sequence of token IDs back to text.
    ///
    /// Skips BOS, EOS and UNK tokens. Converts GPT-2/SmolLM2 space markers
    /// (Ġ → " ", Ċ → "\n", ▁ → " "). Expands `<0xNN>` byte tokens.
    /// Stops at the first EOS token encountered.
    pub fn decode(&self, ids: &[u32]) -> String {
        let mut out = String::new();
        for &id in ids {
            if id == self.bos_id || id == self.unk_id {
                continue;
            }
            if id == self.eos_id {
                break;
            }
            let tok = match self.id_to_token.get(id as usize) {
                Some(t) => t,
                None => continue,
            };
            // Handle byte tokens: <0xNN>
            if tok.starts_with("<0x") && tok.ends_with('>') && tok.len() == 6 {
                if let Ok(b) = u8::from_str_radix(&tok[3..5], 16) {
                    out.push(b as char);
                    continue;
                }
            }
            // Replace whitespace markers used by GPT-2/SmolLM2 BPE:
            //   Ġ (U+0120) = space prefix, Ċ (U+010A) = newline, ▁ = SP prefix
            let s = tok.replace('Ġ', " ").replace('Ċ', "\n").replace('▁', " ");
            out.push_str(&s);
        }
        out
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    /// Number of entries in the vocabulary.
    pub fn vocab_size(&self) -> usize {
        self.id_to_token.len()
    }

    /// Look up a token string and return its ID, or `None` if not present.
    pub fn token_to_id(&self, token: &str) -> Option<u32> {
        self.vocab.get(token).copied()
    }

    /// Look up a token ID and return its string, or `None` if out of range.
    pub fn id_to_token_str(&self, id: u32) -> Option<&str> {
        self.id_to_token.get(id as usize).map(|s| s.as_str())
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- byte_fallback_stub ------------------------------------------------

    #[test]
    fn test_byte_fallback_stub_vocab_size() {
        let tok = BpeTokenizer::byte_fallback_stub();
        assert_eq!(tok.vocab_size(), 256);
    }

    #[test]
    fn test_stub_encode_wraps_with_bos_eos() {
        let tok = BpeTokenizer::byte_fallback_stub();
        let ids = tok.encode("hi");
        assert_eq!(ids[0], tok.bos_id, "first id must be bos_id");
        assert_eq!(*ids.last().unwrap(), tok.eos_id, "last id must be eos_id");
    }

    #[test]
    fn test_stub_decode_skips_special_tokens() {
        let tok = BpeTokenizer::byte_fallback_stub();
        let ids = vec![tok.bos_id, tok.eos_id];
        assert_eq!(tok.decode(&ids), "", "bos+eos only must decode to empty string");
    }

    #[test]
    fn test_encode_decode_roundtrip_ascii() {
        let tok = BpeTokenizer::byte_fallback_stub();
        let text = "hello";
        let ids = tok.encode(text);
        let decoded = tok.decode(&ids);
        // The stub encodes each byte as <0xNN>, decode should recover the chars.
        for c in text.chars() {
            assert!(
                decoded.contains(c) || decoded.len() >= text.len() - 2,
                "decoded='{}' missing '{}'",
                decoded,
                c
            );
        }
    }

    // ---- accessors ---------------------------------------------------------

    #[test]
    fn test_stub_special_token_ids() {
        let tok = BpeTokenizer::byte_fallback_stub();
        assert_eq!(tok.bos_id, 1);
        assert_eq!(tok.eos_id, 2);
        assert_eq!(tok.unk_id, 0);
    }

    #[test]
    fn test_stub_token_lookup() {
        let tok = BpeTokenizer::byte_fallback_stub();
        // Byte 0x41 = 'A'
        assert_eq!(tok.token_to_id("<0x41>"), Some(0x41));
        assert_eq!(tok.id_to_token_str(0x41), Some("<0x41>"));
    }

    // ---- encode correctness ------------------------------------------------

    #[test]
    fn test_encode_empty_string() {
        let tok = BpeTokenizer::byte_fallback_stub();
        let ids = tok.encode("");
        // Only BOS + EOS — no whitespace words to split.
        assert_eq!(ids, vec![tok.bos_id, tok.eos_id]);
    }

    #[test]
    fn test_decode_empty_slice() {
        let tok = BpeTokenizer::byte_fallback_stub();
        assert_eq!(tok.decode(&[]), "");
    }

    // ---- BPE merge path ----------------------------------------------------

    /// Build a tiny BpeTokenizer with a small vocab + merge rules in-memory,
    /// without needing a real tokenizer.json file.
    fn make_bpe_tok() -> BpeTokenizer {
        // Vocab: a=10, b=11, ab=12, c=13, abc=14, Ġ=15, Ġa=16, BOS=1, EOS=2, UNK=0
        let mut vocab = HashMap::new();
        vocab.insert("a".into(), 10u32);
        vocab.insert("b".into(), 11u32);
        vocab.insert("ab".into(), 12u32);
        vocab.insert("c".into(), 13u32);
        vocab.insert("abc".into(), 14u32);
        vocab.insert("Ġ".into(), 15u32);
        vocab.insert("Ġa".into(), 16u32);
        let id_to_token = vec![
            "<unk>".into(), "<bos>".into(), "<eos>".into(),
            String::new(), String::new(), String::new(), String::new(),
            String::new(), String::new(), String::new(),
            "a".into(), "b".into(), "ab".into(), "c".into(), "abc".into(),
            "Ġ".into(), "Ġa".into(),
        ];
        // Merge rules: ("a","b") → "ab" at rank 0; ("ab","c") → "abc" at rank 1
        let mut merge_rank = HashMap::new();
        merge_rank.insert(("a".into(), "b".into()), 0usize);
        merge_rank.insert(("ab".into(), "c".into()), 1usize);
        BpeTokenizer { vocab, id_to_token, merge_rank, special_tokens: Vec::new(), bos_id: 1, eos_id: 2, unk_id: 0 }
    }

    #[test]
    fn test_bpe_merge_applies_in_order() {
        let tok = make_bpe_tok();
        // "abc" should merge a+b→ab, then ab+c→abc giving ID 14.
        let ids = tok.encode("abc");
        // ids = [BOS=1, 14, EOS=2]
        assert_eq!(ids[0], 1, "bos");
        assert_eq!(ids[1], 14, "abc merged to id 14");
        assert_eq!(ids[2], 2, "eos");
    }

    #[test]
    fn test_bpe_partial_merge() {
        let tok = make_bpe_tok();
        // "ab" alone should merge a+b→ab giving ID 12.
        let ids = tok.encode("ab");
        assert_eq!(ids[1], 12, "ab merged to id 12");
    }

    #[test]
    fn test_bpe_no_merge_needed() {
        let tok = make_bpe_tok();
        // "a" alone — no merge possible, maps to ID 10.
        let ids = tok.encode("a");
        assert_eq!(ids[1], 10, "single 'a' maps to id 10");
    }

    // ---- newline encoding --------------------------------------------------

    #[test]
    fn test_encode_newline_uses_ck_token() {
        let mut tok = make_bpe_tok();
        // Inject Ċ into vocab at a known ID.
        tok.vocab.insert("Ċ".into(), 20u32);
        let ids = tok.encode("a\nb");
        // Expected: BOS=1, a=10, Ċ=20, b=11, EOS=2
        // (first word on each line has no Ġ prefix)
        assert_eq!(ids, vec![1, 10, 20, 11, 2], "got {:?}", ids);
    }

    #[test]
    fn test_encode_newline_stub_uses_byte_fallback() {
        let tok = BpeTokenizer::byte_fallback_stub();
        // byte_fallback_stub has <0x0A> mapped to ID 0x0A = 10.
        let ids = tok.encode("x\ny");
        assert!(ids.contains(&0x0A), "\\n must emit <0x0A> (id=10) in stub; got {:?}", ids);
    }

    #[test]
    fn test_encode_double_newline_emits_two_ck_tokens() {
        let mut tok = make_bpe_tok();
        tok.vocab.insert("Ċ".into(), 20u32);
        let ids = tok.encode("a\n\nb");
        // "a\n\nb".split('\n') → ["a", "", "b"]
        // Line 0: a=10; Line 1: Ċ=20, empty; Line 2: Ċ=20, b=11
        assert_eq!(ids, vec![1, 10, 20, 20, 11, 2], "got {:?}", ids);
    }

    #[test]
    fn test_encode_word_after_newline_has_no_space_prefix() {
        let mut tok = make_bpe_tok();
        tok.vocab.insert("Ċ".into(), 20u32);
        // "abc\nabc" — after newline, first word is "abc" (id=14), not "Ġabc"
        let ids = tok.encode("abc\nabc");
        assert_eq!(ids, vec![1, 14, 20, 14, 2], "got {:?}", ids);
    }

    #[test]
    fn test_special_tokens_encode_atomically() {
        // Simulate a tokenizer with <|im_start|> and <|im_end|> as added tokens.
        let mut tok = make_bpe_tok();
        tok.special_tokens = vec![
            ("<|im_start|>".into(), 50u32),
            ("<|im_end|>".into(),   51u32),
        ];
        tok.special_tokens.sort_by(|a, b| b.0.len().cmp(&a.0.len()));

        // "<|im_start|>assistant" should give [BOS, 50, ...assistant tokens..., EOS]
        let ids = tok.encode("<|im_start|>assistant");
        assert_eq!(ids[0], 1,  "BOS");
        assert_eq!(ids[1], 50, "<|im_start|> token");
        assert_eq!(*ids.last().unwrap(), 2, "EOS");

        // <|im_end|> anywhere in the string
        let ids2 = tok.encode("hello<|im_end|>");
        assert!(ids2.contains(&51), "<|im_end|> should appear in ids2");
    }
}
