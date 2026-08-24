/// Returns the heading text if `line` is a Markdown heading (any depth 1-6), else None.
fn heading_text(line: &str) -> Option<&str> {
    let trimmed = line.trim();
    let hashes = trimmed.chars().take_while(|&c| c == '#').count();
    if hashes == 0 || hashes > 6 {
        return None;
    }
    let rest = &trimmed[hashes..];
    rest.strip_prefix(' ').map(|s| s.trim())
}

pub fn extract_section(content: &str, section_name: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    let mut result = Vec::new();
    let mut in_section = false;
    let target = section_name.trim().to_lowercase();

    for line in &lines {
        if let Some(heading) = heading_text(line) {
            if heading_matches(heading, &target) {
                in_section = true;
                result.push(line.to_string());
                continue;
            } else if in_section {
                break;
            }
        }

        if in_section {
            result.push(line.to_string());
        }
    }

    result.join("\n")
}

/// Does a Markdown heading identify the requested section?
///
/// Matching is tolerant of leading decoration (emoji/symbols the aggregator
/// prepends, e.g. `🔴 Do Now (Quick Wins)`) and of a trailing parenthetical
/// or descriptive suffix, while still being strict on word boundaries so
/// `"Do"` does NOT match `"Don't Do This"`. Rule: strip leading
/// non-alphanumeric characters, lowercase, then accept an exact match or a
/// `target + " "` prefix (word-boundary safe).
fn heading_matches(heading: &str, target: &str) -> bool {
    let normalized = heading
        .trim_start_matches(|c: char| !c.is_alphanumeric())
        .trim()
        .to_lowercase();
    normalized == *target || normalized.starts_with(&format!("{} ", target))
}

/// Truncate string safely at char boundary to avoid UTF-8 panic.
pub fn safe_truncate(s: &str, max_chars: usize) -> &str {
    if s.len() <= max_chars {
        return s;
    }
    let mut boundary = max_chars;
    while boundary > 0 && !s.is_char_boundary(boundary) {
        boundary -= 1;
    }
    &s[..boundary]
}
