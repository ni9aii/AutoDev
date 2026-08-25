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
    // Depth of the heading that opened the section; the section ends only at
    // a heading of the same or shallower depth (a `### Fix N:` item inside a
    // `## Do Now` section must NOT terminate it).
    let mut section_depth = 0usize;
    let target = section_name.trim().to_lowercase();

    for line in &lines {
        if let Some(heading) = heading_text(line) {
            let depth = line.chars().take_while(|&c| c == '#').count();
            if !in_section && heading_matches(heading, &target) {
                in_section = true;
                section_depth = depth;
                result.push(line.to_string());
                continue;
            } else if in_section {
                if depth <= section_depth {
                    break;
                }
                // Deeper heading inside the section: keep it as content.
                result.push(line.to_string());
                continue;
            }
        }

        if in_section {
            result.push(line.to_string());
        }
    }

    result.join("\n")
}

/// Extract the CHANGELOG.md section for `version`: from the
/// `## [<version>]` heading up to the next `##` heading or EOF. Returns
/// `None` when no section with exactly that version exists — the release
/// phase gates on this so a release body is always curated, never
/// auto-generated.
pub fn extract_changelog_section(content: &str, version: &str) -> Option<String> {
    let header_prefix = format!("## [{}]", version);
    let mut result: Vec<&str> = Vec::new();
    let mut in_section = false;
    for line in content.lines() {
        let trimmed = line.trim_end();
        if trimmed.starts_with("## ") {
            if in_section {
                break;
            }
            if trimmed.starts_with(&header_prefix) {
                // Exact bracket match only: "[1.2.3]" must not match
                // "[1.2.30]". The prefix already ends at the closing
                // bracket, so what follows must be whitespace or nothing
                // (a date suffix like " - 2026-08-25" is fine).
                let after = &trimmed[header_prefix.len()..];
                if after.is_empty() || after.starts_with(char::is_whitespace) {
                    in_section = true;
                    result.push(trimmed);
                }
            }
            continue;
        }
        if in_section {
            result.push(trimmed);
        }
    }
    if !in_section {
        return None;
    }
    Some(result.join("\n").trim().to_string())
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

/// Truncate string safely to at most `max_chars` **characters** (Unicode
/// scalar values), never splitting a multibyte character.
pub fn safe_truncate(s: &str, max_chars: usize) -> &str {
    if s.chars().count() <= max_chars {
        return s;
    }
    s.char_indices()
        .nth(max_chars)
        .map_or(s, |(idx, _)| &s[..idx])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_changelog_section_found_with_date_suffix() {
        let content = "# Changelog\n\n## [Unreleased]\n\n- wip\n\n## [1.2.3] - 2026-08-25\n\n### Added\n- thing\n\n## [1.2.2] - 2026-08-01\n\n- old\n";
        let sec = extract_changelog_section(content, "1.2.3").unwrap();
        assert!(sec.starts_with("## [1.2.3]"));
        assert!(sec.contains("### Added"));
        assert!(sec.contains("- thing"));
        assert!(!sec.contains("old"), "must stop at the next ## heading");
        assert!(!sec.contains("wip"), "must not leak other sections");
    }

    #[test]
    fn test_extract_changelog_section_runs_to_eof() {
        let content = "## [0.9.0]\n\n- last section";
        let sec = extract_changelog_section(content, "0.9.0").unwrap();
        assert_eq!(sec, "## [0.9.0]\n\n- last section");
    }

    #[test]
    fn test_extract_changelog_section_missing_returns_none() {
        let content = "## [1.2.3] - 2026-08-25\n\n- thing\n";
        assert!(extract_changelog_section(content, "9.9.9").is_none());
    }

    #[test]
    fn test_extract_changelog_section_exact_version_no_prefix_match() {
        // "[1.2.3]" must not match "[1.2.30]".
        let content = "## [1.2.30] - date\n\n- wrong\n";
        assert!(extract_changelog_section(content, "1.2.3").is_none());
    }

    #[test]
    fn test_extract_section_keeps_deeper_headings_inside_section() {
        // Regression (architecture plan Task 1): a `### Fix N:` item heading
        // inside the `## Do Now` section must NOT terminate it. Previously any
        // heading ended the section, so real aggregator plans yielded a
        // header-only section and execute found zero fixes.
        let content = "\
## Summary

## 🔴 Do Now (Quick Wins)

### Fix 1: Alpha

**Description:**
body one

### Fix 2: Beta

body two

## 🟡 Defer to Next Phase

### Deferred 1: Gamma
";
        let section = extract_section(content, "Do Now");
        assert!(section.contains("### Fix 1: Alpha"), "item heading lost");
        assert!(section.contains("Alpha") && section.contains("Beta"));
        assert!(!section.contains("Gamma"), "section bled into Defer");
    }

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
    fn test_safe_truncate_multibyte_counts_chars() {
        // Russian: each char is 2 bytes. Truncating to 5 CHARS must yield
        // exactly 5 characters (10 bytes), not 5 bytes (2.5 chars).
        let s = "привет мир";
        let truncated = safe_truncate(s, 5);
        assert_eq!(truncated.chars().count(), 5);
        assert_eq!(truncated, "приве");
        assert!(s.starts_with(truncated));
    }

    #[test]
    fn test_safe_truncate_exact_char_count_returns_whole_string() {
        let s = "привет";
        assert_eq!(safe_truncate(s, 6), s);
        assert_eq!(safe_truncate(s, 7), s);
    }
}
