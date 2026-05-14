# Document Conversion Patterns

## HTML to PDF Conversion on Arch Linux

### Problem
Need to convert formatted HTML documents to PDF for official/legal submission.

### Available Tools (checked in priority order)

| Tool | Availability | Quality | Notes |
|------|-------------|---------|-------|
| **wkhtmltopdf** | Not installed | High | Best for programmatic conversion. Install: `sudo pacman -S wkhtmltopdf` |
| **LibreOffice** | Not installed | Medium | Can convert via CLI: `libreoffice --headless --convert-to pdf`. Install: `sudo pacman -S libreoffice` |
| **Chromium/Chrome** | Not installed | High | Can use `--print-to-pdf` flag or Playwright. Install: `sudo pacman -S chromium` |
| **OnlyOffice (flatpak)** | Installed | Medium | Has CLI but blocked in headless mode. GUI works fine |
| **Playwright** | Not installed | High | Python library for browser automation. Install: `pip install playwright` |
| **weasyprint** | Not installed | High | Pure Python HTML→PDF. Install: `pip install weasyprint` |

### Recommended Approach

**For programmatic conversion (scripts):**
```bash
# Install wkhtmltopdf (best balance of quality and simplicity)
sudo pacman -S wkhtmltopdf

# Convert
wkhtmltopdf --page-size A4 --margin-top 25 --margin-bottom 20 --margin-left 30 --margin-right 20 input.html output.pdf
```

**For manual conversion (one-off):**
1. Open HTML in browser: `xdg-open document.html`
2. Press Ctrl+P
3. Select "Save as PDF"
4. Set margins appropriately

### HTML Formatting for Print

Key CSS for legal/official documents:
```css
@page {
    size: A4;
    margin: 2.5cm 2cm 2cm 3cm; /* top right bottom left */
}
body {
    font-family: "Times New Roman", Times, serif;
    font-size: 12pt;
    line-height: 1.5;
    text-align: justify;
}
p {
    text-indent: 36px; /* First line indent */
    margin: 0;
}
```

### User Preference

User prefers documents formatted for Russian legal standards:
- Times New Roman 12pt
- 1.5 line spacing
- Justified text
- First line indent
- Standard A4 margins
