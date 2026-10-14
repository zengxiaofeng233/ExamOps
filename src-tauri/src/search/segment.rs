//! Text segmentation for SQLite FTS5.
//!
//! FTS5's `unicode61` tokenizer treats a run of Chinese characters as one huge
//! token, so a search for "导数" would never match "函数的导数". We therefore
//! index CJK text one character per token ("函 数 的 导 数") and turn queries
//! into phrase queries over those characters:
//!
//! * keyword search (`QueryMode::All`): every term must occur as an exact
//!   substring (`"导 数"`), Latin words also match by prefix (`"deriv"*`);
//! * question retrieval (`QueryMode::Any`): overlapping CJK bigrams and words
//!   are OR-ed and ranked by BM25, which works without a Chinese dictionary.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueryMode {
    All,
    /// Question retrieval; used by the Context Engine (Stage 3).
    #[cfg_attr(not(test), allow(dead_code))]
    Any,
}

pub fn is_cjk(c: char) -> bool {
    matches!(c as u32,
        0x3040..=0x30FF       // Hiragana, Katakana
        | 0x31F0..=0x31FF     // Katakana extensions
        | 0x3400..=0x4DBF     // CJK extension A
        | 0x4E00..=0x9FFF     // CJK unified ideographs
        | 0xAC00..=0xD7AF     // Hangul syllables
        | 0xF900..=0xFAFF     // CJK compatibility ideographs
        | 0xFF66..=0xFF9F     // Half-width katakana
        | 0x20000..=0x2FA1F)  // CJK extensions B-F
}

/// Full-width ASCII → half-width, ideographic space → space, lowercase.
pub fn normalize_char(c: char) -> char {
    let c = match c as u32 {
        0xFF01..=0xFF5E => char::from_u32(c as u32 - 0xFEE0).unwrap_or(c),
        0x3000 => ' ',
        _ => c,
    };
    c.to_lowercase().next().unwrap_or(c)
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() && !is_cjk(c)
}

/// Text stored in the FTS table: CJK characters separated by spaces.
pub fn index_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    for raw in text.chars() {
        let c = normalize_char(raw);
        if is_cjk(c) {
            out.push(' ');
            out.push(c);
            out.push(' ');
        } else if is_word_char(c) {
            out.push(c);
        } else {
            out.push(' ');
        }
    }
    out
}

#[derive(Debug, Clone, PartialEq)]
struct Token {
    text: String,
    cjk: bool,
}

/// Splits input into terms (separated by whitespace/punctuation), each a token list.
fn terms(input: &str) -> Vec<Vec<Token>> {
    let mut all = Vec::new();
    let mut current: Vec<Token> = Vec::new();
    let mut word = String::new();
    let flush_word = |word: &mut String, current: &mut Vec<Token>| {
        if !word.is_empty() {
            current.push(Token { text: std::mem::take(word), cjk: false });
        }
    };
    for raw in input.chars() {
        let c = normalize_char(raw);
        if is_cjk(c) {
            flush_word(&mut word, &mut current);
            current.push(Token { text: c.to_string(), cjk: true });
        } else if is_word_char(c) {
            word.push(c);
        } else {
            flush_word(&mut word, &mut current);
            if !current.is_empty() {
                all.push(std::mem::take(&mut current));
            }
        }
    }
    flush_word(&mut word, &mut current);
    if !current.is_empty() {
        all.push(current);
    }
    all
}

const STOP_CHARS: &str = "的了吗呢吧啊呀么是在和与及或都也还就而但并把被给着过得我你他她它们这那哪请问之其の은는이가をはがにへでとも";
const STOP_BIGRAMS: &[&str] = &[
    "什么", "怎么", "如何", "为何", "哪些", "一下", "解释", "说明", "介绍", "讲讲", "可以", "能否", "是否",
    "有没", "没有", "告诉", "帮我", "为什", "一个", "这个", "那个",
];
const STOP_WORDS: &[&str] = &[
    "the", "a", "an", "is", "are", "was", "were", "be", "of", "to", "in", "on", "for", "and", "or",
    "what", "how", "why", "which", "who", "does", "do", "did", "can", "could", "explain", "please",
    "me", "about", "with", "this", "that", "it", "its", "as", "by", "at", "from", "tell", "give",
];

fn quote(phrase: &str) -> String {
    format!("\"{}\"", phrase.replace('"', "\"\""))
}

/// Builds an FTS5 MATCH expression, or `None` when nothing searchable remains.
pub fn fts_query(input: &str, mode: QueryMode) -> Option<String> {
    let mut pieces: Vec<String> = Vec::new();
    let mut push = |piece: String| {
        if !pieces.contains(&piece) && pieces.len() < 40 {
            pieces.push(piece);
        }
    };
    for term in terms(input) {
        match mode {
            QueryMode::All => {
                let phrase: Vec<&str> = term.iter().map(|t| t.text.as_str()).collect();
                let last = term.last().expect("terms are non-empty");
                let prefix = !last.cjk && last.text.chars().count() >= 3;
                push(format!("{}{}", quote(&phrase.join(" ")), if prefix { "*" } else { "" }));
            }
            QueryMode::Any => {
                let mut run: Vec<char> = Vec::new();
                let flush_run = |run: &mut Vec<char>, push: &mut dyn FnMut(String)| {
                    let mut produced = false;
                    for pair in run.windows(2) {
                        let bigram: String = pair.iter().collect();
                        if pair.iter().any(|c| STOP_CHARS.contains(*c)) || STOP_BIGRAMS.contains(&bigram.as_str()) {
                            continue;
                        }
                        push(quote(&format!("{} {}", pair[0], pair[1])));
                        produced = true;
                    }
                    if !produced {
                        for c in run.iter().filter(|c| !STOP_CHARS.contains(**c)) {
                            push(quote(&c.to_string()));
                        }
                    }
                    run.clear();
                };
                for token in &term {
                    if token.cjk {
                        run.extend(token.text.chars());
                    } else {
                        flush_run(&mut run, &mut push);
                        let text = token.text.as_str();
                        let numeric = text.chars().all(|c| c.is_ascii_digit());
                        if text.chars().count() >= 2 && !numeric && !STOP_WORDS.contains(&text) {
                            let prefix = text.chars().count() >= 4;
                            push(format!("{}{}", quote(text), if prefix { "*" } else { "" }));
                        }
                    }
                }
                flush_run(&mut run, &mut push);
            }
        }
    }
    if pieces.is_empty() {
        return None;
    }
    let joiner = if mode == QueryMode::All { " AND " } else { " OR " };
    Some(pieces.join(joiner))
}

/// Lower-cased strings to highlight in results: whole terms for keyword search,
/// bigrams and words for question retrieval.
pub fn highlight_terms(input: &str, mode: QueryMode) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for term in terms(input) {
        let mut add = |s: String| {
            if !s.is_empty() && !out.contains(&s) {
                out.push(s);
            }
        };
        match mode {
            QueryMode::All => {
                // Highlight CJK runs and words separately (they are adjacent in the source).
                let mut buffer = String::new();
                let mut buffer_cjk = None;
                for token in &term {
                    if buffer_cjk.is_some() && buffer_cjk != Some(token.cjk) {
                        add(std::mem::take(&mut buffer));
                    }
                    buffer.push_str(&token.text);
                    buffer_cjk = Some(token.cjk);
                }
                add(buffer);
            }
            QueryMode::Any => {
                let chars: Vec<char> = term.iter().filter(|t| t.cjk).flat_map(|t| t.text.chars()).collect();
                for pair in chars.windows(2) {
                    if !pair.iter().any(|c| STOP_CHARS.contains(*c)) {
                        add(pair.iter().collect());
                    }
                }
                for token in term.iter().filter(|t| !t.cjk) {
                    if token.text.chars().count() >= 2 && !STOP_WORDS.contains(&token.text.as_str()) {
                        add(token.text.clone());
                    }
                }
            }
        }
    }
    out
}

#[derive(Debug, Clone, serde::Serialize, PartialEq)]
pub struct Segment {
    pub text: String,
    pub hit: bool,
}

/// Char ranges where any term occurs (case-insensitive), merged.
pub fn hit_ranges(content: &str, terms: &[String]) -> Vec<(usize, usize)> {
    let lowered: Vec<char> = content.chars().map(normalize_char).collect();
    let mut ranges: Vec<(usize, usize)> = Vec::new();
    for term in terms {
        let needle: Vec<char> = term.chars().collect();
        if needle.is_empty() || needle.len() > lowered.len() {
            continue;
        }
        let mut i = 0;
        while i + needle.len() <= lowered.len() {
            if lowered[i..i + needle.len()] == needle[..] {
                ranges.push((i, i + needle.len()));
                i += needle.len();
            } else {
                i += 1;
            }
        }
    }
    ranges.sort();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in ranges {
        match merged.last_mut() {
            Some(last) if start <= last.1 => last.1 = last.1.max(end),
            _ => merged.push((start, end)),
        }
    }
    merged
}

/// A short excerpt around the first hit, split into highlighted segments.
pub fn snippet(content: &str, terms: &[String], max_chars: usize) -> Vec<Segment> {
    let chars: Vec<char> = content
        .chars()
        .map(|c| if c == '\n' || c == '\r' || c == '\t' { ' ' } else { c })
        .collect();
    let ranges = hit_ranges(content, terms);
    let start = ranges.first().map(|r| r.0.saturating_sub(max_chars / 4)).unwrap_or(0);
    let end = (start + max_chars).min(chars.len());
    let mut segments = Vec::new();
    let mut push = |text: String, hit: bool| {
        if text.is_empty() {
            return;
        }
        match segments.last_mut() {
            Some(Segment { text: last, hit: last_hit }) if *last_hit == hit => last.push_str(&text),
            _ => segments.push(Segment { text, hit }),
        }
    };
    if start > 0 {
        push("…".into(), false);
    }
    let mut cursor = start;
    for (hit_start, hit_end) in ranges.into_iter().filter(|r| r.1 > start && r.0 < end) {
        let s = hit_start.max(start);
        let e = hit_end.min(end);
        push(chars[cursor..s].iter().collect(), false);
        push(chars[s..e].iter().collect(), true);
        cursor = e;
    }
    push(chars[cursor..end].iter().collect(), false);
    if end < chars.len() {
        push("…".into(), false);
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn indexes_cjk_one_character_per_token() {
        assert_eq!(
            index_text("函数f(x)的导数").split_whitespace().collect::<Vec<_>>(),
            vec!["函", "数", "f", "x", "的", "导", "数"]
        );
        assert_eq!(index_text("ＡＢＣ　１２"), "abc 12");
    }

    #[test]
    fn keyword_queries_are_exact_phrases() {
        assert_eq!(fts_query("导数", QueryMode::All).unwrap(), "\"导 数\"");
        assert_eq!(
            fts_query("第3章 Derivative", QueryMode::All).unwrap(),
            "\"第 3 章\" AND \"derivative\"*"
        );
        assert_eq!(fts_query("  ，。!!  ", QueryMode::All), None);
        // Quotes and FTS operators in user input cannot break the expression.
        assert_eq!(fts_query("a\" OR b", QueryMode::All).unwrap(), "\"a\" AND \"or\" AND \"b\"");
    }

    #[test]
    fn question_queries_use_bigrams_without_function_words() {
        let query = fts_query("什么是导数的几何意义？", QueryMode::Any).unwrap();
        assert!(query.contains("\"导 数\""));
        assert!(query.contains("\"几 何\""));
        assert!(query.contains("\"意 义\""));
        assert!(!query.contains("什 么"));
        assert!(!query.contains("数 的"));
        assert!(query.contains(" OR "));
        let english = fts_query("What is the chain rule?", QueryMode::Any).unwrap();
        assert_eq!(english, "\"chain\"* OR \"rule\"*");
        assert_eq!(fts_query("的", QueryMode::Any), None);
    }

    #[test]
    fn snippets_highlight_terms_case_insensitively() {
        let terms = highlight_terms("导数 limit", QueryMode::All);
        assert_eq!(terms, vec!["导数".to_string(), "limit".to_string()]);
        let segments = snippet("函数的导数与Limit的关系", &terms, 100);
        let hits: Vec<_> = segments.iter().filter(|s| s.hit).map(|s| s.text.as_str()).collect();
        assert_eq!(hits, vec!["导数", "Limit"]);
        let long = format!("{}导数{}", "前".repeat(200), "后".repeat(200));
        let cut = snippet(&long, &terms, 60);
        assert_eq!(cut.first().unwrap().text.chars().next(), Some('…'));
        assert!(cut.iter().any(|s| s.hit && s.text == "导数"));
        assert!(cut.last().unwrap().text.ends_with('…'));
    }
}
