use crate::index::{Symbol, SymbolKind};

#[derive(Clone, Copy)]
struct Heading {
    level: usize,
    start: usize,
    line_end: usize,
    section_end: usize,
}

pub(super) fn extract_headings(source: &[u8]) -> Vec<Symbol> {
    let mut headings = collect_headings(source);

    for i in 0..headings.len() {
        headings[i].section_end = source.len();
        for j in i + 1..headings.len() {
            if headings[j].level <= headings[i].level {
                headings[i].section_end = headings[j].start;
                break;
            }
        }
    }

    headings
        .into_iter()
        .filter_map(|heading| {
            let signature = String::from_utf8_lossy(&source[heading.start..heading.line_end])
                .trim()
                .to_string();
            let name = heading_name(&signature)?;
            Some(Symbol {
                name,
                kind: SymbolKind::Heading,
                signature,
                byte_range: (heading.start, heading.section_end),
                is_test: false,
            })
        })
        .collect()
}

fn collect_headings(source: &[u8]) -> Vec<Heading> {
    let mut headings = Vec::new();
    let mut offset = 0;
    let mut in_fence = false;
    let mut fence_marker = 0;
    let mut fence_len = 0;

    while offset < source.len() {
        let line_end = source[offset..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(source.len(), |pos| offset + pos);
        let line = trim_cr(&source[offset..line_end]);
        let trimmed = trim_ascii_start(line);

        if let Some((marker, len)) = fence_start(trimmed) {
            if in_fence {
                if marker == fence_marker && len >= fence_len {
                    in_fence = false;
                }
            } else {
                in_fence = true;
                fence_marker = marker;
                fence_len = len;
            }
        } else if !in_fence
            && let Some(level) = atx_heading_level(trimmed) {
                headings.push(Heading {
                    level,
                    start: offset,
                    line_end,
                    section_end: source.len(),
                });
            }

        offset = if line_end < source.len() { line_end + 1 } else { source.len() };
    }

    headings
}

fn atx_heading_level(line: &[u8]) -> Option<usize> {
    let level = line.iter().take_while(|&&b| b == b'#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    if line.len() > level && !line[level].is_ascii_whitespace() {
        return None;
    }
    Some(level)
}

fn heading_name(signature: &str) -> Option<String> {
    let trimmed = signature.trim_start();
    let content = trimmed.trim_start_matches('#').trim();
    let content = content
        .strip_suffix('#')
        .map_or(content, |s| s.trim_end_matches('#').trim_end());
    if content.is_empty() {
        None
    } else {
        Some(content.to_string())
    }
}

fn fence_start(line: &[u8]) -> Option<(u8, usize)> {
    let marker = *line.first()?;
    if marker != b'`' && marker != b'~' {
        return None;
    }
    let len = line.iter().take_while(|&&b| b == marker).count();
    (len >= 3).then_some((marker, len))
}

fn trim_ascii_start(bytes: &[u8]) -> &[u8] {
    let leading = bytes.iter().take_while(|b| b.is_ascii_whitespace()).count();
    &bytes[leading..]
}

fn trim_cr(bytes: &[u8]) -> &[u8] {
    bytes.strip_suffix(b"\r").unwrap_or(bytes)
}

