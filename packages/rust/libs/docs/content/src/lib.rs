pub mod config;
pub use config::*;

pub struct DocPage {
    pub slug: &'static str,
    pub title: &'static str,
    pub category: &'static str,
    pub description: &'static str,
    pub order: u32,
    pub markdown: &'static str,
}

pub struct DocCategory {
    pub name: &'static str,
    pub icon: &'static str,
    pub pages: Vec<&'static DocPage>,
}

pub static OVERVIEW: DocPage = DocPage {
    slug: "overview",
    title: "Overview & Philosophy",
    category: "Getting Started",
    description: "The Bloomery philosophy: why pure Nix derivations and zero Cargo overhead matter.",
    order: 1,
    markdown: include_str!("docs/overview.md"),
};

pub static QUICKSTART: DocPage = DocPage {
    slug: "quickstart",
    title: "Quickstart Guide",
    category: "Getting Started",
    description: "Initialize your Rust flake and build your first binary in under 60 seconds.",
    order: 2,
    markdown: include_str!("docs/quickstart.md"),
};

pub static PROFILES: DocPage = DocPage {
    slug: "profiles",
    title: "Strongly-Typed Profiles",
    category: "Configuration",
    description: "Evaluate LTO, codegen-units, panic strategies, and CPU targets with typed Nix modules.",
    order: 3,
    markdown: include_str!("docs/profiles.md"),
};

pub static API: DocPage = DocPage {
    slug: "api",
    title: "mkFlake API Reference",
    category: "Configuration",
    description: "Categorized config.toml reference for source, toolchain, profile, flags, and checks.",
    order: 4,
    markdown: include_str!("docs/api.md"),
};

pub static OVERRIDES: DocPage = DocPage {
    slug: "overrides",
    title: "Colocated Overrides",
    category: "Configuration",
    description: "Configure package-level filesets, system dependencies, and compiler flags in overrides.nix.",
    order: 5,
    markdown: include_str!("docs/overrides.md"),
};

pub static ARCHITECTURE: DocPage = DocPage {
    slug: "architecture",
    title: "Architecture & Internals",
    category: "Getting Started",
    description: "Deep dive into zero-IFD feature resolution, per-crate DAGs, and build script sandboxes.",
    order: 3,
    markdown: include_str!("docs/architecture.md"),
};

pub static MATRIX: DocPage = DocPage {
    slug: "matrix",
    title: "Verification Matrix",
    category: "Validation & Benchmarks",
    description: "Overview of Bloomery's 6 dedicated test workspaces running 68+ CI checks.",
    order: 7,
    markdown: include_str!("docs/matrix.md"),
};

pub static BENCHMARKS: DocPage = DocPage {
    slug: "benchmarks",
    title: "Architectural Benchmarks",
    category: "Validation & Benchmarks",
    description: "Performance comparisons and architectural advantages over Crane and Cargo2nix.",
    order: 8,
    markdown: include_str!("docs/benchmarks.md"),
};

pub static ALL_PAGES: &[&DocPage] = &[
    &OVERVIEW,
    &QUICKSTART,
    &ARCHITECTURE,
    &PROFILES,
    &API,
    &OVERRIDES,
    &MATRIX,
    &BENCHMARKS,
];

pub fn all_pages() -> &'static [&'static DocPage] {
    ALL_PAGES
}

pub fn get_page(slug: &str) -> Option<&'static DocPage> {
    ALL_PAGES.iter().copied().find(|p| p.slug == slug)
}

pub fn categories() -> Vec<DocCategory> {
    let mut map: Vec<(&'static str, &'static str, Vec<&'static DocPage>)> = vec![
        ("Getting Started", "🚀", Vec::new()),
        ("Configuration", "⚙️", Vec::new()),
        ("Validation & Benchmarks", "📊", Vec::new()),
    ];

    for page in ALL_PAGES {
        if let Some((_, _, pages)) = map.iter_mut().find(|(cat, _, _)| *cat == page.category) {
            pages.push(page);
        }
    }

    map.into_iter()
        .map(|(name, icon, pages)| DocCategory { name, icon, pages })
        .collect()
}

/// Render a markdown string into semantic HTML.
pub fn render_markdown(input: &str) -> String {
    let lines: Vec<&str> = input.lines().collect();
    MarkdownRenderer::new(input.len() * 2).render(&lines)
}

struct MarkdownRenderer {
    output: String,
    in_code_block: bool,
    in_list: bool,
    in_table: bool,
    in_blockquote: bool,
}

impl MarkdownRenderer {
    fn new(capacity: usize) -> Self {
        Self {
            output: String::with_capacity(capacity),
            in_code_block: false,
            in_list: false,
            in_table: false,
            in_blockquote: false,
        }
    }

    fn render(mut self, lines: &[&str]) -> String {
        let mut index = 0;
        while index < lines.len() {
            let line = lines[index];
            let trimmed = line.trim();
            if self.render_fence(trimmed) {
                index += 1;
                continue;
            }
            if self.in_code_block {
                self.render_code_line(line);
                index += 1;
                continue;
            }
            if self.render_horizontal_rule(trimmed) {
                index += 1;
                continue;
            }
            if self.render_blockquote(trimmed) || self.render_heading(trimmed) {
                index += 1;
                continue;
            }
            if self.render_table_line(trimmed, lines, &mut index)
                || self.render_list_line(line, trimmed, &mut index)
            {
                continue;
            }
            if trimmed.is_empty() {
                self.close_for_empty_line();
                index += 1;
                continue;
            }
            self.render_paragraph(trimmed);
            index += 1;
        }
        self.close_all();
        self.output
    }

    fn render_fence(&mut self, trimmed: &str) -> bool {
        if !trimmed.starts_with("```") {
            return false;
        }
        if self.in_code_block {
            self.output.push_str("</code></pre></div>\n");
            self.in_code_block = false;
            return true;
        }
        self.close_for_fence_or_rule();
        // Only the first info-string token is the language; any trailing role
        // token is consumed by DOCS/CONSISTENCY and does not label the block.
        let info = trimmed.trim_start_matches('`').trim();
        let lang = info.split_whitespace().next().unwrap_or("");
        let lang = if lang.is_empty() { "code" } else { lang };
        self.output.push_str(&format!(
            "<div class=\"code-block-wrapper\"><div class=\"code-header\"><span class=\"code-lang\">{lang}</span><button class=\"copy-btn\" onclick=\"navigator.clipboard.writeText(this.closest('.code-block-wrapper').querySelector('code').innerText);this.innerText='Copied!';setTimeout(()=>this.innerText='Copy',2000)\">Copy</button></div><pre><code class=\"language-{lang}\">"
        ));
        self.in_code_block = true;
        true
    }

    fn render_code_line(&mut self, line: &str) {
        self.output.push_str(&escape_html(line));
        self.output.push('\n');
    }

    fn render_horizontal_rule(&mut self, trimmed: &str) -> bool {
        if trimmed != "---" && trimmed != "***" {
            return false;
        }
        self.close_for_fence_or_rule();
        self.output.push_str("<hr class=\"divider\" />\n");
        true
    }

    fn render_blockquote(&mut self, trimmed: &str) -> bool {
        if trimmed.starts_with("> [!") {
            if self.in_blockquote {
                self.output.push_str("</div>\n");
            }
            let alert_tag = trimmed
                .trim_start_matches("> [!")
                .trim_end_matches(']')
                .to_lowercase();
            let (alert_class, alert_title) = alert_style(&alert_tag);
            self.output.push_str(&format!(
                "<div class=\"alert {alert_class}\"><div class=\"alert-title\">{alert_title}</div><div class=\"alert-body\">"
            ));
            self.in_blockquote = true;
            return true;
        }
        if trimmed.starts_with('>') {
            if !self.in_blockquote {
                self.output
                    .push_str("<div class=\"alert alert-note\"><div class=\"alert-body\">");
                self.in_blockquote = true;
            }
            self.output
                .push_str(&render_inline(trimmed.trim_start_matches('>').trim()));
            self.output.push(' ');
            return true;
        }
        if self.in_blockquote && trimmed.is_empty() {
            self.output.push_str("</div></div>\n");
            self.in_blockquote = false;
            return true;
        }
        false
    }

    fn render_heading(&mut self, trimmed: &str) -> bool {
        if !trimmed.starts_with('#') {
            return false;
        }
        self.close_for_heading_or_empty();
        let level = trimmed
            .chars()
            .take_while(|&character| character == '#')
            .count();
        let text = trimmed.trim_start_matches('#').trim();
        let id = slugify(text);
        let rendered = render_inline(text);
        self.output.push_str(&format!(
            "<h{level} id=\"{id}\" class=\"doc-heading doc-h{level}\"><a href=\"#{id}\" class=\"heading-anchor\">#</a>{rendered}</h{level}>\n"
        ));
        true
    }

    fn render_table_line(&mut self, trimmed: &str, lines: &[&str], index: &mut usize) -> bool {
        if !trimmed.starts_with('|') || !trimmed.ends_with('|') {
            if self.in_table {
                self.output.push_str("</tbody></table></div>\n");
                self.in_table = false;
            }
            return false;
        }
        if self.in_table {
            self.render_table_row(trimmed);
            *index += 1;
            return true;
        }
        if self.in_list {
            self.close_list();
        }
        self.output
            .push_str("<div class=\"table-container\"><table class=\"data-table\"><thead><tr>");
        for header in table_cells(trimmed) {
            self.output
                .push_str(&format!("<th>{}</th>", render_inline(header)));
        }
        self.output.push_str("</tr></thead><tbody>\n");
        if *index + 1 < lines.len()
            && lines[*index + 1].trim().starts_with('|')
            && lines[*index + 1].contains("---")
        {
            *index += 1;
        }
        self.in_table = true;
        *index += 1;
        true
    }

    fn render_table_row(&mut self, trimmed: &str) {
        self.output.push_str("<tr>");
        for cell in table_cells(trimmed) {
            self.output
                .push_str(&format!("<td>{}</td>", render_inline(cell)));
        }
        self.output.push_str("</tr>\n");
    }

    fn render_list_line(&mut self, line: &str, trimmed: &str, index: &mut usize) -> bool {
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if !self.in_list {
                self.output.push_str("<ul class=\"doc-list\">\n");
                self.in_list = true;
            }
            self.output.push_str(&format!(
                "<li>{}</li>\n",
                render_inline(trimmed[2..].trim())
            ));
            *index += 1;
            return true;
        }
        if self.in_list && !trimmed.is_empty() && (line.starts_with("  ") || line.starts_with('\t'))
        {
            let item_text = trimmed.trim_start_matches("- ").trim();
            self.output.push_str(&format!(
                "<li class=\"nested\">{}</li>\n",
                render_inline(item_text)
            ));
            *index += 1;
            return true;
        }
        if self.in_list && trimmed.is_empty() {
            self.close_list();
            *index += 1;
            return true;
        }
        false
    }

    fn render_paragraph(&mut self, trimmed: &str) {
        self.output.push_str("<p class=\"doc-paragraph\">");
        self.output.push_str(&render_inline(trimmed));
        self.output.push_str("</p>\n");
    }

    fn close_for_fence_or_rule(&mut self) {
        self.close_list();
        self.close_table();
        if self.in_blockquote {
            self.output.push_str("</div>\n");
            self.in_blockquote = false;
        }
    }

    fn close_for_heading_or_empty(&mut self) {
        self.close_list();
        self.close_table();
        if self.in_blockquote {
            self.output.push_str("</div></div>\n");
            self.in_blockquote = false;
        }
    }

    fn close_for_empty_line(&mut self) {
        self.close_for_heading_or_empty();
    }

    fn close_list(&mut self) {
        if self.in_list {
            self.output.push_str("</ul>\n");
            self.in_list = false;
        }
    }

    fn close_table(&mut self) {
        if self.in_table {
            self.output.push_str("</tbody></table></div>\n");
            self.in_table = false;
        }
    }

    fn close_all(&mut self) {
        if self.in_code_block {
            self.output.push_str("</code></pre></div>\n");
        }
        self.close_list();
        self.close_table();
        if self.in_blockquote {
            self.output.push_str("</div></div>\n");
        }
    }
}

fn alert_style(tag: &str) -> (&'static str, &'static str) {
    match tag {
        "tip" => ("alert-tip", "💡 Tip"),
        "warning" => ("alert-warning", "⚠️ Warning"),
        "important" => ("alert-important", "⚡ Important"),
        _ => ("alert-note", "ℹ️ Note"),
    }
}

fn table_cells(line: &str) -> Vec<&str> {
    line.split('|')
        .filter(|cell| !cell.is_empty())
        .map(str::trim)
        .collect()
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn slugify(s: &str) -> String {
    let clean: String = s
        .chars()
        .map(|c| {
            if c.is_alphanumeric() {
                c.to_ascii_lowercase()
            } else if c == ' ' || c == '-' || c == '_' {
                '-'
            } else {
                ' '
            }
        })
        .collect();
    clean.split_whitespace().collect::<Vec<&str>>().join("-")
}

#[allow(clippy::collapsible_if)]
fn render_inline(text: &str) -> String {
    let mut out = String::with_capacity(text.len() * 2);
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut idx = 0;

    while idx < len {
        // Inline code `code`
        if chars[idx] == '`' {
            if let Some(end) = chars[idx + 1..].iter().position(|&c| c == '`') {
                let code_content: String = chars[idx + 1..idx + 1 + end].iter().collect();
                out.push_str("<code class=\"inline-code\">");
                out.push_str(&escape_html(&code_content));
                out.push_str("</code>");
                idx += end + 2;
                continue;
            }
        }

        // Bold **text**
        if idx + 1 < len && chars[idx] == '*' && chars[idx + 1] == '*' {
            if let Some(end) = (idx + 2..len - 1).find(|&j| chars[j] == '*' && chars[j + 1] == '*')
            {
                let bold_content: String = chars[idx + 2..end].iter().collect();
                out.push_str("<strong>");
                out.push_str(&render_inline(&bold_content));
                out.push_str("</strong>");
                idx = end + 2;
                continue;
            }
        }

        // Link [text](url)
        if chars[idx] == '[' {
            if let Some(bracket_end) = chars[idx + 1..].iter().position(|&c| c == ']') {
                let text_end = idx + 1 + bracket_end;
                if text_end + 1 < len && chars[text_end + 1] == '(' {
                    if let Some(paren_end) = chars[text_end + 2..].iter().position(|&c| c == ')') {
                        let link_text: String = chars[idx + 1..text_end].iter().collect();
                        let link_url: String = chars[text_end + 2..text_end + 2 + paren_end]
                            .iter()
                            .collect();
                        out.push_str(&format!(
                            "<a href=\"{}\" class=\"doc-link\">{}</a>",
                            escape_html(&link_url),
                            render_inline(&link_text)
                        ));
                        idx = text_end + 2 + paren_end + 1;
                        continue;
                    }
                }
            }
        }

        // Escaped HTML character
        match chars[idx] {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
        idx += 1;
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONTENT-CATALOG-001"))]
    #[cfg_attr(any(), bloomery("DOCS-CONTENT-CATALOG-003"))]
    #[cfg_attr(any(), bloomery("DOCS-CONTENT-CATALOG-004"))]
    #[cfg_attr(any(), bloomery("DOCS-CONTENT-CATALOG-005"))]
    #[cfg_attr(any(), bloomery("DOCS-CONTENT-CATALOG-006"))]
    fn every_catalog_page_has_stable_slugs_and_complete_metadata() {
        let pages = all_pages();
        let expected_slugs = [
            "overview",
            "quickstart",
            "architecture",
            "profiles",
            "api",
            "overrides",
            "matrix",
            "benchmarks",
        ];
        let actual_slugs = pages.iter().map(|page| page.slug).collect::<Vec<_>>();
        let unique_slugs = actual_slugs.iter().copied().collect::<BTreeSet<_>>();

        assert_eq!(actual_slugs, expected_slugs);
        assert_eq!(unique_slugs.len(), pages.len());
        assert_eq!(
            pages.iter().map(|page| page.order).collect::<Vec<_>>(),
            vec![1, 2, 3, 3, 4, 5, 7, 8]
        );
        for page in pages {
            assert!(
                !page.title.trim().is_empty(),
                "missing title for {}",
                page.slug
            );
            assert!(
                [
                    "Getting Started",
                    "Configuration",
                    "Validation & Benchmarks"
                ]
                .contains(&page.category),
                "unknown category for {}",
                page.slug
            );
            assert!(
                !page.description.trim().is_empty(),
                "missing description for {}",
                page.slug
            );
            assert!(
                !page.markdown.trim().is_empty(),
                "missing content for {}",
                page.slug
            );
        }
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONTENT-CATALOG-002"))]
    fn every_catalog_page_is_retrievable_by_its_slug() {
        for page in all_pages() {
            let found = get_page(page.slug).expect("catalog slug should resolve");
            assert!(std::ptr::eq(found, *page));
        }
        assert!(get_page("not-a-documentation-page").is_none());
    }

    #[test]
    fn test_markdown_renderer() {
        let md = "# Title\n\nSome **bold** text and `inline_code`.\n\n- item 1\n- item 2\n";
        let html = render_markdown(md);
        assert!(html.contains("<h1"));
        assert!(html.contains("<strong>bold</strong>"));
        assert!(html.contains("<code class=\"inline-code\">inline_code</code>"));
        assert!(html.contains("<li>item 1</li>"));
    }

    #[test]
    #[cfg_attr(any(), bloomery("DOCS-CONTENT-CATALOG-007"))]
    fn code_fence_role_tokens_are_not_displayed() {
        let html =
            render_markdown("```toml bloomery-config\n[build]\nprofileName = \"release\"\n```\n");
        assert!(html.contains("class=\"language-toml\""), "{html}");
        assert!(
            html.contains("<span class=\"code-lang\">toml</span>"),
            "{html}"
        );
        assert!(!html.contains("bloomery-config"), "{html}");
    }
}
