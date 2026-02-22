use serde::Serialize;
use std::collections::HashMap;
use crate::scanner::ScannerError;

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub doc_id: String,
    pub doc_name: String,
    pub score: f64,
    pub snippet: String,
}

/// Simple TF-IDF based semantic search over a corpus of OCR texts.
pub fn semantic_search(
    query: &str,
    corpus: &[(String, String, String)], // (doc_id, doc_name, ocr_text)
    max_results: usize,
) -> Vec<SearchResult> {
    if corpus.is_empty() || query.trim().is_empty() {
        return Vec::new();
    }

    let query_terms = tokenize(query);
    if query_terms.is_empty() {
        return Vec::new();
    }

    // Build document frequency (DF) for each term
    let mut df: HashMap<String, usize> = HashMap::new();
    let doc_count = corpus.len();

    let tokenized_docs: Vec<Vec<String>> = corpus
        .iter()
        .map(|(_, _, text)| tokenize(text))
        .collect();

    for doc_tokens in &tokenized_docs {
        let unique: std::collections::HashSet<&String> = doc_tokens.iter().collect();
        for term in unique {
            *df.entry(term.clone()).or_insert(0) += 1;
        }
    }

    // Compute TF-IDF scores for each document
    let mut results: Vec<SearchResult> = Vec::new();

    for (i, (doc_id, doc_name, text)) in corpus.iter().enumerate() {
        let doc_tokens = &tokenized_docs[i];
        if doc_tokens.is_empty() {
            continue;
        }

        // TF for this document
        let mut tf: HashMap<&str, f64> = HashMap::new();
        for token in doc_tokens {
            *tf.entry(token.as_str()).or_insert(0.0) += 1.0;
        }
        let max_tf = tf.values().cloned().fold(0.0f64, f64::max).max(1.0);

        // Compute cosine similarity between query and document
        let mut score = 0.0;
        for qt in &query_terms {
            let term_tf = tf.get(qt.as_str()).copied().unwrap_or(0.0) / max_tf;
            let term_df = df.get(qt).copied().unwrap_or(1) as f64;
            let idf = (doc_count as f64 / term_df).ln() + 1.0;
            score += term_tf * idf;
        }

        if score > 0.0 {
            // Generate snippet: find best matching sentence
            let snippet = generate_snippet(text, &query_terms);

            results.push(SearchResult {
                doc_id: doc_id.clone(),
                doc_name: doc_name.clone(),
                score: (score * 1000.0).round() / 1000.0,
                snippet,
            });
        }
    }

    // Sort by score descending
    results.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
    results.truncate(max_results);

    results
}

fn tokenize(text: &str) -> Vec<String> {
    text.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 2)
        .map(|w| w.to_string())
        .collect()
}

fn generate_snippet(text: &str, query_terms: &[String]) -> String {
    let sentences: Vec<&str> = text.split(|c: char| c == '.' || c == '\n')
        .filter(|s| !s.trim().is_empty())
        .collect();

    if sentences.is_empty() {
        return text.chars().take(200).collect();
    }

    // Score each sentence by how many query terms it contains
    let mut best_score = 0;
    let mut best_idx = 0;

    for (i, sentence) in sentences.iter().enumerate() {
        let lower = sentence.to_lowercase();
        let score: usize = query_terms
            .iter()
            .filter(|qt| lower.contains(qt.as_str()))
            .count();
        if score > best_score {
            best_score = score;
            best_idx = i;
        }
    }

    let snippet = sentences[best_idx].trim();
    if snippet.len() > 200 {
        format!("{}...", &snippet[..200])
    } else {
        snippet.to_string()
    }
}
