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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_section_found() {
        let content = "# Plan\n\n## Do Now\n- Fix 1\n- Fix 2\n\n## Defer\n- Fix 3";
        let section = extract_section(content, "Do Now");
        assert!(section.contains("Fix 1"));
        assert!(section.contains("Fix 2"));
        assert!(!section.contains("Fix 3"));
    }

    #[test]
    fn test_extract_section_not_found() {
        let content = "# Plan\n\n## Other\n- Something";
        let section = extract_section(content, "Do Now");
        assert!(section.is_empty());
    }

    #[test]
    fn test_extract_section_any_heading_depth() {
        let content = "# Plan\n\n### Do Now\n- Fix 1\n\n### Defer\n- Fix 2";
        let section = extract_section(content, "Do Now");
        assert!(section.contains("Fix 1"));
        assert!(!section.contains("Fix 2"));
    }

    #[test]
    fn test_extract_section_exact_match_not_substring() {
        let content = "# Plan\n\n## Don't Do This\n- Fix 1\n\n## Do\n- Fix 2";
        let section = extract_section(content, "Do");
        assert!(!section.contains("Fix 1"));
        assert!(section.contains("Fix 2"));
    }

    #[test]
    fn test_extract_section_matches_aggregator_decorated_heading() {
        // Regression: review-aggregator emits "## 🔴 Do Now (Quick Wins)".
        // The execute phase calls extract_section(plan, "Do Now"); a strict
        // whole-heading equality check silently missed this, so execute found
        // zero fixes on real aggregator output.
        let content =
            "# Auto-Dev Fix Plan\n\n## 🔴 Do Now (Quick Wins)\n- Fix A\n- Fix B\n\n## 🟡 Defer\n- Fix C";
        let section = extract_section(content, "Do Now");
        assert!(
            section.contains("Fix A"),
            "decorated 'Do Now' heading not matched"
        );
        assert!(section.contains("Fix B"));
        assert!(!section.contains("Fix C"), "section bled into Defer");
    }

    #[test]
    fn test_safe_truncate_ascii() {
        assert_eq!(safe_truncate("hello world", 5), "hello");
    }

    #[test]
    fn test_safe_truncate_multibyte() {
        // Russian: each char is 2 bytes
        let s = "привет";
        let truncated = safe_truncate(s, 5);
        assert!(truncated.len() <= 5);
        assert!(s.starts_with(truncated));
    }
}
