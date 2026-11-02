// Track B — Speed Demon Hackathon (Index)
// Full-text search index and query evaluation over legal text corpus.
//
// Zero external dependencies. Fast, byte-exact, deterministic.

use std::collections::{HashMap, HashSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::PathBuf;
use std::sync::Arc;
use std::thread;

#[inline]
fn is_token_byte(b: u8) -> bool {
    (b >= b'a' && b <= b'z') || (b >= b'0' && b <= b'9') || (b >= b'A' && b <= b'Z')
}

#[inline]
fn to_lower_ascii(b: u8) -> u8 {
    if b >= b'A' && b <= b'Z' {
        b + 32
    } else {
        b
    }
}

/// Tokenize text into lowercase token byte sequences
fn tokenize_text(text: &[u8], mut callback: impl FnMut(&[u8])) {
    let mut i = 0;
    let len = text.len();
    let mut scratch = Vec::with_capacity(32);

    while i < len {
        while i < len && !is_token_byte(text[i]) {
            i += 1;
        }
        let start = i;
        let mut has_upper = false;
        while i < len && is_token_byte(text[i]) {
            if text[i] >= b'A' && text[i] <= b'Z' {
                has_upper = true;
            }
            i += 1;
        }
        if start < i {
            let raw = &text[start..i];
            if !has_upper {
                callback(raw);
            } else {
                scratch.clear();
                for &b in raw {
                    scratch.push(to_lower_ascii(b));
                }
                callback(&scratch);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum BoolOp {
    And,
    Or,
    Not,
}

#[derive(Clone, Debug)]
enum Query {
    Bool {
        terms: Vec<Vec<u8>>,
        ops: Vec<BoolOp>,
    },
    Phrase {
        terms: Vec<Vec<u8>>,
    },
}

fn parse_query_line(line: &[u8]) -> Option<Query> {
    if line.starts_with(b"BOOL ") {
        let rest = &line[5..];
        let parts: Vec<&[u8]> = rest.split(|&b| b == b' ').collect();
        if parts.is_empty() {
            return None;
        }
        let mut terms = Vec::new();
        let mut ops = Vec::new();
        for (idx, &part) in parts.iter().enumerate() {
            if idx % 2 == 0 {
                terms.push(part.to_vec());
            } else {
                let op = match part {
                    b"AND" => BoolOp::And,
                    b"OR" => BoolOp::Or,
                    b"NOT" => BoolOp::Not,
                    _ => return None,
                };
                ops.push(op);
            }
        }
        Some(Query::Bool { terms, ops })
    } else if line.starts_with(b"PHRASE ") {
        let rest = &line[7..];
        if rest.len() < 2 || rest[0] != b'"' || rest[rest.len() - 1] != b'"' {
            return None;
        }
        let inner = &rest[1..rest.len() - 1];
        let terms = inner.split(|&b| b == b' ').map(|s| s.to_vec()).collect();
        Some(Query::Phrase { terms })
    } else {
        None
    }
}

fn intersect(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len().min(b.len()));
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push(a[i]);
            i += 1;
            j += 1;
        } else if a[i] < b[j] {
            i += 1;
        } else {
            j += 1;
        }
    }
    out
}

fn union(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len() + b.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            out.push(a[i]);
            i += 1;
            j += 1;
        } else if a[i] < b[j] {
            out.push(a[i]);
            i += 1;
        } else {
            out.push(b[j]);
            j += 1;
        }
    }
    if i < a.len() {
        out.extend_from_slice(&a[i..]);
    }
    if j < b.len() {
        out.extend_from_slice(&b[j..]);
    }
    out
}

fn difference(a: &[u32], b: &[u32]) -> Vec<u32> {
    let mut out = Vec::with_capacity(a.len());
    let (mut i, mut j) = (0, 0);
    while i < a.len() && j < b.len() {
        if a[i] == b[j] {
            i += 1;
            j += 1;
        } else if a[i] < b[j] {
            out.push(a[i]);
            i += 1;
        } else {
            j += 1;
        }
    }
    if i < a.len() {
        out.extend_from_slice(&a[i..]);
    }
    out
}

fn format_result(docs: &[u32]) -> String {
    if docs.is_empty() {
        return "0 0\n".to_string();
    }
    let count = docs.len();
    let sum: u64 = docs.iter().map(|&d| d as u64).sum();
    let top10_len = count.min(10);
    let mut s = format!("{} {}", count, sum);
    for &d in &docs[..top10_len] {
        s.push(' ');
        s.push_str(&d.to_string());
    }
    s.push('\n');
    s
}

struct Engine {
    docs_data: Arc<Vec<u8>>,
    doc_offsets: Arc<Vec<(usize, usize)>>,
    term_to_id: Arc<HashMap<Vec<u8>, usize>>,
    postings: Arc<Vec<Vec<u32>>>,
}

impl Engine {
    fn build(docs_bytes: Vec<u8>, queries: &[Query]) -> Self {
        let mut query_terms = HashSet::new();
        for q in queries {
            match q {
                Query::Bool { terms, .. } => {
                    for t in terms {
                        query_terms.insert(t.clone());
                    }
                }
                Query::Phrase { terms } => {
                    for t in terms {
                        query_terms.insert(t.clone());
                    }
                }
            }
        }

        let mut term_to_id = HashMap::with_capacity(query_terms.len());
        for (id, term) in query_terms.into_iter().enumerate() {
            term_to_id.insert(term, id);
        }

        let num_terms = term_to_id.len();
        let mut postings: Vec<Vec<u32>> = vec![Vec::new(); num_terms];
        let mut last_doc: Vec<u32> = vec![u32::MAX; num_terms];
        let mut doc_offsets: Vec<(usize, usize)> = Vec::with_capacity(250_000);

        let mut pos = 0;
        let len = docs_bytes.len();

        while pos < len {
            let mut line_end = pos;
            while line_end < len && docs_bytes[line_end] != b'\n' {
                line_end += 1;
            }

            let line = &docs_bytes[pos..line_end];
            if !line.is_empty() {
                // Find tab
                if let Some(tab_idx) = line.iter().position(|&b| b == b'\t') {
                    let doc_id = doc_offsets.len() as u32;
                    let text_start = pos + tab_idx + 1;
                    let text_end = line_end;
                    doc_offsets.push((text_start, text_end));

                    let text_slice = &docs_bytes[text_start..text_end];
                    tokenize_text(text_slice, |tok| {
                        if let Some(&tid) = term_to_id.get(tok) {
                            if last_doc[tid] != doc_id {
                                postings[tid].push(doc_id);
                                last_doc[tid] = doc_id;
                            }
                        }
                    });
                }
            }

            pos = line_end + 1;
        }

        Engine {
            docs_data: Arc::new(docs_bytes),
            doc_offsets: Arc::new(doc_offsets),
            term_to_id: Arc::new(term_to_id),
            postings: Arc::new(postings),
        }
    }

    fn execute(&self, query: &Query) -> Vec<u32> {
        match query {
            Query::Bool { terms, ops } => {
                if terms.is_empty() {
                    return Vec::new();
                }
                let mut current = if let Some(&tid) = self.term_to_id.get(&terms[0]) {
                    self.postings[tid].clone()
                } else {
                    Vec::new()
                };

                for (idx, &op) in ops.iter().enumerate() {
                    let next_term = &terms[idx + 1];
                    let next_postings = if let Some(&tid) = self.term_to_id.get(next_term) {
                        &self.postings[tid][..]
                    } else {
                        &[][..]
                    };

                    current = match op {
                        BoolOp::And => intersect(&current, next_postings),
                        BoolOp::Or => union(&current, next_postings),
                        BoolOp::Not => difference(&current, next_postings),
                    };
                }
                current
            }
            Query::Phrase { terms } => {
                if terms.is_empty() {
                    return Vec::new();
                }
                for t in terms {
                    if !self.term_to_id.contains_key(t) {
                        return Vec::new();
                    }
                }

                // Intersect postings anchor-first (smallest posting list first)
                let mut term_indices: Vec<usize> = (0..terms.len()).collect();
                term_indices.sort_by_key(|&idx| {
                    let tid = self.term_to_id[&terms[idx]];
                    self.postings[tid].len()
                });

                let first_tid = self.term_to_id[&terms[term_indices[0]]];
                let mut candidates = self.postings[first_tid].clone();

                for &idx in &term_indices[1..] {
                    if candidates.is_empty() {
                        break;
                    }
                    let tid = self.term_to_id[&terms[idx]];
                    candidates = intersect(&candidates, &self.postings[tid]);
                }

                if candidates.is_empty() {
                    return Vec::new();
                }

                // Verify exact phrase match on candidate documents
                let k = terms.len();
                let mut matching = Vec::with_capacity(candidates.len());
                let mut ring: Vec<Vec<u8>> = (0..k).map(|_| Vec::with_capacity(32)).collect();

                for doc_id in candidates {
                    let (start, end) = self.doc_offsets[doc_id as usize];
                    let text = &self.docs_data[start..end];

                    let mut count = 0;
                    let mut matched = false;

                    tokenize_text(text, |tok| {
                        if matched {
                            return;
                        }
                        let idx = count % k;
                        ring[idx].clear();
                        ring[idx].extend_from_slice(tok);
                        count += 1;
                        if count >= k {
                            let mut ok = true;
                            for m in 0..k {
                                let r_idx = (count + m) % k;
                                if ring[r_idx] != terms[m] {
                                    ok = false;
                                    break;
                                }
                            }
                            if ok {
                                matched = true;
                            }
                        }
                    });

                    if matched {
                        matching.push(doc_id);
                    }
                }
                matching
            }
        }
    }
}

fn solve(docs_bytes: Vec<u8>, queries_bytes: &[u8]) -> Vec<u8> {
    let mut queries = Vec::new();
    for line in queries_bytes.split(|&b| b == b'\n') {
        let line = if line.ends_with(b"\r") {
            &line[..line.len() - 1]
        } else {
            line
        };
        if !line.is_empty() {
            if let Some(q) = parse_query_line(line) {
                queries.push(q);
            }
        }
    }

    let engine = Arc::new(Engine::build(docs_bytes, &queries));
    let num_queries = queries.len();

    // Multi-threaded parallel query execution (12 threads for 6 P-cores)
    let num_threads = 12.min(num_queries).max(1);
    let chunk_size = (num_queries + num_threads - 1) / num_threads;
    let queries = Arc::new(queries);

    let mut handles = Vec::with_capacity(num_threads);
    for t in 0..num_threads {
        let start = t * chunk_size;
        let end = (start + chunk_size).min(num_queries);
        if start >= end {
            continue;
        }

        let engine = Arc::clone(&engine);
        let queries = Arc::clone(&queries);

        handles.push(thread::spawn(move || {
            let mut out = String::with_capacity((end - start) * 32);
            for i in start..end {
                let res = engine.execute(&queries[i]);
                out.push_str(&format_result(&res));
            }
            out
        }));
    }

    let mut result_string = String::with_capacity(num_queries * 32);
    for h in handles {
        let chunk_out = h.join().expect("thread panicked");
        result_string.push_str(&chunk_out);
    }

    result_string.into_bytes()
}

fn locate_file(paths: &[&str]) -> Option<PathBuf> {
    for p in paths {
        let pb = PathBuf::from(p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    None
}

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // Check for single-query mode:
    // e.g. track-b --docs <docs.txt> --query <query> [--json]
    let mut docs_arg: Option<PathBuf> = None;
    let mut queries_arg: Option<PathBuf> = None;
    let mut out_arg: Option<PathBuf> = None;
    let mut single_query: Option<String> = None;
    let mut json_output = false;

    let mut i = 1;
    let mut positional = Vec::new();
    while i < args.len() {
        if args[i] == "--docs" && i + 1 < args.len() {
            docs_arg = Some(PathBuf::from(&args[i + 1]));
            i += 2;
        } else if args[i] == "--query" && i + 1 < args.len() {
            single_query = Some(args[i + 1].clone());
            i += 2;
        } else if args[i] == "--queries" && i + 1 < args.len() {
            queries_arg = Some(PathBuf::from(&args[i + 1]));
            i += 2;
        } else if args[i] == "--out" && i + 1 < args.len() {
            out_arg = Some(PathBuf::from(&args[i + 1]));
            i += 2;
        } else if args[i] == "--json" {
            json_output = true;
            i += 1;
        } else {
            positional.push(args[i].clone());
            i += 1;
        }
    }

    if docs_arg.is_none() && !positional.is_empty() {
        docs_arg = Some(PathBuf::from(&positional[0]));
    }
    if queries_arg.is_none() && positional.len() > 1 {
        queries_arg = Some(PathBuf::from(&positional[1]));
    }
    if out_arg.is_none() && positional.len() > 2 {
        out_arg = Some(PathBuf::from(&positional[2]));
    }

    let docs_path = docs_arg.unwrap_or_else(|| {
        locate_file(&["/data/docs.txt", "/data/b/docs.txt"])
            .unwrap_or_else(|| PathBuf::from("/data/docs.txt"))
    });

    let docs_bytes = fs::read(&docs_path).unwrap_or_else(|e| {
        eprintln!("failed to read docs at {}: {}", docs_path.display(), e);
        std::process::exit(1);
    });

    if let Some(query_str) = single_query {
        let q_bytes = query_str.into_bytes();
        let query_obj = match parse_query_line(&q_bytes) {
            Some(q) => q,
            None => {
                eprintln!(
                    "failed to parse query: {}",
                    std::str::from_utf8(&q_bytes).unwrap_or("")
                );
                std::process::exit(1);
            }
        };

        let start = std::time::Instant::now();
        let engine = Engine::build(docs_bytes, &[query_obj.clone()]);
        let res = engine.execute(&query_obj);
        let elapsed_us = start.elapsed().as_micros();

        let count = res.len();
        let sum: u64 = res.iter().map(|&d| d as u64).sum();

        if json_output {
            let ids_json = res
                .iter()
                .take(100)
                .map(|id| id.to_string())
                .collect::<Vec<_>>()
                .join(",");
            println!(
                "{{\"count\":{},\"sum\":{},\"latency_us\":{},\"docs\":[{}]}}",
                count, sum, elapsed_us, ids_json
            );
        } else {
            print!("{}", format_result(&res));
        }
        return;
    }

    let queries_path = queries_arg.unwrap_or_else(|| {
        locate_file(&["/data/queries.txt", "/data/b/queries.txt"])
            .unwrap_or_else(|| PathBuf::from("/data/queries.txt"))
    });

    let queries_bytes = fs::read(&queries_path).unwrap_or_else(|e| {
        eprintln!(
            "failed to read queries at {}: {}",
            queries_path.display(),
            e
        );
        std::process::exit(1);
    });

    let out_bytes = solve(docs_bytes, &queries_bytes);

    let out_path = out_arg.unwrap_or_else(|| PathBuf::from("/out/results.txt"));
    if let Some(parent) = out_path.parent() {
        fs::create_dir_all(parent).ok();
    }
    let out_file = File::create(&out_path).unwrap_or_else(|e| {
        eprintln!(
            "failed to create output file at {}: {}",
            out_path.display(),
            e
        );
        std::process::exit(1);
    });
    let mut writer = BufWriter::new(out_file);
    writer
        .write_all(&out_bytes)
        .expect("failed to write output");
    writer.flush().expect("failed to flush output");
}

#[cfg(test)]
mod tests {
    use super::*;

    const EXAMPLE_DOCS: &[u8] = b"0\tThe surety posted bail; the BAIL BOND was forfeited.\n1\tWrit of Habeas-Corpus: see 42 U.S.C. \xc2\xa7 1983 (bail bond).\n2\t\n3\tZ\xc3\xbcrich bail\xe2\x80\x94bond, bail bail bond. Case 007.\n4\tIn the matter of the estate. Bond not posted, No. 7.\n";

    const EXAMPLE_QUERIES: &[u8] = b"BOOL bail AND bond\nBOOL bail\nBOOL estate OR surety AND bail\nBOOL bond NOT bail\nBOOL bail NOT bail\nBOOL zurich\nBOOL rich AND z\nBOOL not AND bond\nBOOL 7\nBOOL the OR writ OR case\nPHRASE \"bail bond\"\nPHRASE \"bail bail bond\"\nPHRASE \"bond bail\"\nPHRASE \"habeas corpus\"\nPHRASE \"u s c 1983\"\nPHRASE \"estate bond\"\nPHRASE \"forfeited writ\"\nPHRASE \"in the matter of\"\n";

    const EXAMPLE_RESULTS: &[u8] = b"3 4 0 1 3\n3 4 0 1 3\n1 0 0\n1 4 4\n0 0\n0 0\n1 3 3\n1 4 4\n1 4 4\n4 8 0 1 3 4\n3 4 0 1 3\n1 3 3\n1 3 3\n1 1 1\n1 1 1\n1 4 4\n0 0\n1 4 4\n";

    #[test]
    fn test_tokenizer() {
        let mut tokens = Vec::new();
        tokenize_text(b"Z\xc3\xbcrich bail-bond", |tok| {
            tokens.push(tok.to_vec());
        });
        assert_eq!(
            tokens,
            vec![
                b"z".to_vec(),
                b"rich".to_vec(),
                b"bail".to_vec(),
                b"bond".to_vec()
            ]
        );
    }

    #[test]
    fn test_worked_example() {
        let got = solve(EXAMPLE_DOCS.to_vec(), EXAMPLE_QUERIES);
        assert_eq!(
            std::str::from_utf8(&got).unwrap(),
            std::str::from_utf8(EXAMPLE_RESULTS).unwrap()
        );
    }
}
