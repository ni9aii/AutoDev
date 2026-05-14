#!/usr/bin/env python3
"""
Review Aggregator for Auto-Dev Pipeline
Aggregates findings from 4 reviewers and generates prioritized fix plan.
"""

import argparse
import json
import os
import re
import sys
from collections import defaultdict
from datetime import datetime
from pathlib import Path


def parse_review_file(filepath):
    """Parse a review report markdown file into structured data."""
    findings = []
    current_finding = None
    
    with open(filepath, 'r') as f:
        content = f.read()
    
    # Extract reviewer role from filename
    role = Path(filepath).stem.split('-')[-3]
    
    # Parse findings - look for patterns like:
    # ### [CRITICAL] Issue title
    # Description...
    # - File: `path/to/file`
    # - Line: 42
    
    # Try multiple report formats: markdown headers, table rows, bullet lists
    # See references/iteration-2-patterns.md for format details
    patterns = [
        # Markdown header: ### [CRITICAL] Title
        (r'###\s*\[(CRITICAL|IMPORTANT|MINOR)\]\s*(.+?)\n(.*?)(?=\n#{1,3}\s|\Z)', 3),
        # Table row: | CRITICAL | Title | ... |
        (r'\|\s*(CRITICAL|IMPORTANT|MINOR)\s*\|\s*(.+?)\s*\|', 2),
        # Bullet list: - [CRITICAL] Title
        (r'^\s*[-*]\s*\[(CRITICAL|IMPORTANT|MINOR)\]\s*(.+)$', 2),
    ]
    
    matches = []
    for pattern, expected_groups in patterns:
        found = re.findall(pattern, content, re.DOTALL | re.IGNORECASE | re.MULTILINE)
        for match in found:
            if expected_groups == 3:
                matches.append(match)  # (severity, title, body)
            else:
                # Table/bullet: no body, use empty description
                matches.append((match[0], match[1], ""))
    
    for severity, title, body in matches:
        # Skip self-corrected / false-alarm findings
        body_lower = body.lower()
        skip_markers = [
            'removing this entry', 'downgrading', 'false alarm',
            'not present', 'not a bug', 'not an issue',
            'no critical here', 'no issue here', 're-checking',
        ]
        if any(marker in body_lower for marker in skip_markers):
            continue
        
        finding = {
            'role': role,
            'severity': severity.upper(),
            'title': title.strip(),
            'description': body.strip(),
            'file': None,
            'line': None,
        }
        
        # Extract file path
        file_match = re.search(r'[Ff]ile:\s*`?([^`\n]+)`?', body)
        if file_match:
            finding['file'] = file_match.group(1).strip()
        
        # Extract line number
        line_match = re.search(r'[Ll]ine:\s*(\d+)', body)
        if line_match:
            finding['line'] = int(line_match.group(1))
        
        findings.append(finding)
    
    return findings


def prioritize_findings(findings):
    """Sort findings by severity and role."""
    severity_order = {'CRITICAL': 0, 'IMPORTANT': 1, 'MINOR': 2}
    
    return sorted(
        findings,
        key=lambda f: (severity_order.get(f['severity'], 3), f['role'], f['title'])
    )


def group_by_file(findings):
    """Group findings by file path."""
    groups = defaultdict(list)
    for finding in findings:
        file_key = finding['file'] or 'General'
        groups[file_key].append(finding)
    return dict(groups)


def generate_plan(findings, output_path):
    """Generate markdown implementation plan from findings."""
    prioritized = prioritize_findings(findings)
    by_file = group_by_file(prioritized)
    
    lines = []
    lines.append("# Auto-Dev Fix Plan")
    lines.append(f"\n> Generated: {datetime.now().isoformat()}")
    lines.append(f"> Total findings: {len(findings)}")
    lines.append(f"> Critical: {sum(1 for f in findings if f['severity'] == 'CRITICAL')}")
    lines.append(f"> Important: {sum(1 for f in findings if f['severity'] == 'IMPORTANT')}")
    lines.append(f"> Minor: {sum(1 for f in findings if f['severity'] == 'MINOR')}")
    lines.append("")
    
    # Summary by role
    lines.append("## Summary by Reviewer")
    role_counts = defaultdict(lambda: defaultdict(int))
    for f in findings:
        role_counts[f['role']][f['severity']] += 1
    
    for role, counts in sorted(role_counts.items()):
        lines.append(f"\n### {role.title()} Reviewer")
        for sev in ['CRITICAL', 'IMPORTANT', 'MINOR']:
            if counts[sev] > 0:
                lines.append(f"- {sev}: {counts[sev]}")
    
    lines.append("")
    lines.append("---")
    lines.append("")
    
    # Critical fixes first
    critical = [f for f in prioritized if f['severity'] == 'CRITICAL']
    if critical:
        lines.append("## 🔴 Critical Fixes (Must Fix)")
        lines.append("")
        for i, finding in enumerate(critical, 1):
            lines.append(f"### Fix {i}: {finding['title']}")
            lines.append(f"\n**Source:** {finding['role'].title()} Reviewer")
            if finding['file']:
                lines.append(f"**File:** `{finding['file']}`")
            if finding['line']:
                lines.append(f"**Line:** {finding['line']}")
            lines.append(f"\n**Description:**")
            lines.append(finding['description'])
            lines.append("")
            lines.append("**Action:** _To be filled by implementer_")
            lines.append("")
    
    # Important fixes
    important = [f for f in prioritized if f['severity'] == 'IMPORTANT']
    if important:
        lines.append("## 🟡 Important Fixes (Should Fix)")
        lines.append("")
        for i, finding in enumerate(important, 1):
            lines.append(f"### Fix {i}: {finding['title']}")
            lines.append(f"\n**Source:** {finding['role'].title()} Reviewer")
            if finding['file']:
                lines.append(f"**File:** `{finding['file']}`")
            if finding['line']:
                lines.append(f"**Line:** {finding['line']}")
            lines.append(f"\n**Description:**")
            lines.append(finding['description'])
            lines.append("")
            lines.append("**Action:** _To be filled by implementer_")
            lines.append("")
    
    # Minor fixes
    minor = [f for f in prioritized if f['severity'] == 'MINOR']
    if minor:
        lines.append("## 🟢 Minor Fixes (Nice to Have)")
        lines.append("")
        for i, finding in enumerate(minor, 1):
            lines.append(f"### Fix {i}: {finding['title']}")
            lines.append(f"\n**Source:** {finding['role'].title()} Reviewer")
            if finding['file']:
                lines.append(f"**File:** `{finding['file']}`")
            if finding['line']:
                lines.append(f"**Line:** {finding['line']}")
            lines.append(f"\n**Description:**")
            lines.append(finding['description'])
            lines.append("")
            lines.append("**Action:** _To be filled by implementer_")
            lines.append("")
    
    # Write output
    with open(output_path, 'w') as f:
        f.write('\n'.join(lines))
    
    return output_path


def main():
    parser = argparse.ArgumentParser(description='Aggregate review findings into fix plan')
    parser.add_argument('--input-dir', required=True, help='Directory with review reports')
    parser.add_argument('--output', required=True, help='Output plan file path')
    args = parser.parse_args()
    
    input_dir = Path(args.input_dir)
    if not input_dir.exists():
        print(f"Error: Input directory not found: {input_dir}", file=sys.stderr)
        sys.exit(1)
    
    # Parse all review files
    all_findings = []
    for review_file in sorted(input_dir.glob('*.md')):
        findings = parse_review_file(str(review_file))
        all_findings.extend(findings)
        print(f"Parsed {len(findings)} findings from {review_file.name}")
    
    if not all_findings:
        print("No findings found. Generating empty plan.")
        all_findings = []
    
    # Generate plan
    output_path = generate_plan(all_findings, args.output)
    print(f"Plan generated: {output_path}")
    print(f"Total findings: {len(all_findings)}")
    
    # Summary
    severity_counts = defaultdict(int)
    for f in all_findings:
        severity_counts[f['severity']] += 1
    
    print(f"\nSeverity breakdown:")
    for sev in ['CRITICAL', 'IMPORTANT', 'MINOR']:
        print(f"  {sev}: {severity_counts[sev]}")


if __name__ == '__main__':
    main()
