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
    title: "mkWorkspace API Reference",
    category: "Configuration",
    description: "Categorized options reference for source, toolchain, profile, flags, and checks.",
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
    category: "Architecture",
    description: "Deep dive into zero-IFD feature resolution, per-crate DAGs, and build script sandboxes.",
    order: 6,
    markdown: include_str!("docs/architecture.md"),
};

pub static ALL_PAGES: &[&DocPage] = &[
    &OVERVIEW,
    &QUICKSTART,
    &PROFILES,
    &API,
    &OVERRIDES,
    &ARCHITECTURE,
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
        ("Architecture", "🏗️", Vec::new()),
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
    let mut out = String::with_capacity(input.len() * 2);
    let mut in_code_block = false;
    let mut in_list = false;
    let mut in_table = false;
    let mut in_blockquote = false;

    let lines: Vec<&str> = input.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim();

        // Fenced code blocks
        if trimmed.starts_with("```") {
            if in_code_block {
                out.push_str("</code></pre></div>\n");
                in_code_block = false;
            } else {
                if in_list {
                    out.push_str("</ul>\n");
                    in_list = false;
                }
                if in_table {
                    out.push_str("</tbody></table></div>\n");
                    in_table = false;
                }
                if in_blockquote {
                    out.push_str("</div>\n");
                    in_blockquote = false;
                }

                let lang = trimmed.trim_start_matches('`').trim();
                let lang_label = if lang.is_empty() { "code" } else { lang };
                out.push_str(&format!(
                    "<div class=\"code-block-wrapper\"><div class=\"code-header\"><span class=\"code-lang\">{lang_label}</span><button class=\"copy-btn\" onclick=\"navigator.clipboard.writeText(this.closest('.code-block-wrapper').querySelector('code').innerText);this.innerText='Copied!';setTimeout(()=>this.innerText='Copy',2000)\">Copy</button></div><pre><code class=\"language-{lang}\">"
                ));
                in_code_block = true;
            }
            i += 1;
            continue;
        }

        if in_code_block {
            let escaped = escape_html(line);
            out.push_str(&escaped);
            out.push('\n');
            i += 1;
            continue;
        }

        // Horizontal rules
        if trimmed == "---" || trimmed == "***" {
            if in_list {
                out.push_str("</ul>\n");
                in_list = false;
            }
            if in_table {
                out.push_str("</tbody></table></div>\n");
                in_table = false;
            }
            if in_blockquote {
                out.push_str("</div>\n");
                in_blockquote = false;
            }
            out.push_str("<hr class=\"divider\" />\n");
            i += 1;
            continue;
        }

        // GitHub-style callouts/alerts (> [!NOTE], > [!TIP], etc.)
        if trimmed.starts_with("> [!") {
            if in_blockquote {
                out.push_str("</div>\n");
            }
            let alert_tag = trimmed
                .trim_start_matches("> [!")
                .trim_end_matches(']')
                .to_lowercase();
            let alert_class = match alert_tag.as_str() {
                "tip" => "alert-tip",
                "warning" => "alert-warning",
                "important" => "alert-important",
                _ => "alert-note",
            };
            let alert_title = match alert_tag.as_str() {
                "tip" => "💡 Tip",
                "warning" => "⚠️ Warning",
                "important" => "⚡ Important",
                _ => "ℹ️ Note",
            };
            out.push_str(&format!(
                "<div class=\"alert {alert_class}\"><div class=\"alert-title\">{alert_title}</div><div class=\"alert-body\">"
            ));
            in_blockquote = true;
            i += 1;
            continue;
        } else if trimmed.starts_with(">") {
            let quote_text = trimmed.trim_start_matches('>').trim();
            if !in_blockquote {
                out.push_str("<div class=\"alert alert-note\"><div class=\"alert-body\">");
                in_blockquote = true;
            }
            out.push_str(&render_inline(quote_text));
            out.push(' ');
            i += 1;
            continue;
        } else if in_blockquote && trimmed.is_empty() {
            out.push_str("</div></div>\n");
            in_blockquote = false;
            i += 1;
            continue;
        }

        // Headings
        if trimmed.starts_with('#') {
            if in_list {
                out.push_str("</ul>\n");
                in_list = false;
            }
            if in_table {
                out.push_str("</tbody></table></div>\n");
                in_table = false;
            }
            if in_blockquote {
                out.push_str("</div></div>\n");
                in_blockquote = false;
            }

            let level = trimmed.chars().take_while(|&c| c == '#').count();
            let text = trimmed.trim_start_matches('#').trim();
            let id = slugify(text);
            let rendered = render_inline(text);
            out.push_str(&format!(
                "<h{level} id=\"{id}\" class=\"doc-heading doc-h{level}\"><a href=\"#{id}\" class=\"heading-anchor\">#</a>{rendered}</h{level}>\n"
            ));
            i += 1;
            continue;
        }

        // Tables
        if trimmed.starts_with('|') && trimmed.ends_with('|') {
            if !in_table {
                if in_list {
                    out.push_str("</ul>\n");
                    in_list = false;
                }
                out.push_str(
                    "<div class=\"table-container\"><table class=\"data-table\"><thead><tr>",
                );
                let headers: Vec<&str> = trimmed
                    .split('|')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.trim())
                    .collect();
                for h in headers {
                    out.push_str(&format!("<th>{}</th>", render_inline(h)));
                }
                out.push_str("</tr></thead><tbody>\n");

                // Check for separator line next
                if i + 1 < lines.len()
                    && lines[i + 1].trim().starts_with('|')
                    && lines[i + 1].contains("---")
                {
                    i += 1;
                }
                in_table = true;
                i += 1;
                continue;
            } else {
                out.push_str("<tr>");
                let cells: Vec<&str> = trimmed
                    .split('|')
                    .filter(|s| !s.is_empty())
                    .map(|s| s.trim())
                    .collect();
                for c in cells {
                    out.push_str(&format!("<td>{}</td>", render_inline(c)));
                }
                out.push_str("</tr>\n");
                i += 1;
                continue;
            }
        } else if in_table {
            out.push_str("</tbody></table></div>\n");
            in_table = false;
        }

        // Unordered lists
        if trimmed.starts_with("- ") || trimmed.starts_with("* ") {
            if !in_list {
                out.push_str("<ul class=\"doc-list\">\n");
                in_list = true;
            }
            let item_text = trimmed[2..].trim();
            out.push_str(&format!("<li>{}</li>\n", render_inline(item_text)));
            i += 1;
            continue;
        } else if in_list
            && !trimmed.is_empty()
            && (line.starts_with("  ") || line.starts_with('\t'))
        {
            // Nested or continuation list line
            let item_text = trimmed.trim_start_matches("- ").trim();
            out.push_str(&format!(
                "<li class=\"nested\">{}</li>\n",
                render_inline(item_text)
            ));
            i += 1;
            continue;
        } else if in_list && trimmed.is_empty() {
            out.push_str("</ul>\n");
            in_list = false;
            i += 1;
            continue;
        }

        // Empty lines
        if trimmed.is_empty() {
            if in_list {
                out.push_str("</ul>\n");
                in_list = false;
            }
            if in_table {
                out.push_str("</tbody></table></div>\n");
                in_table = false;
            }
            if in_blockquote {
                out.push_str("</div></div>\n");
                in_blockquote = false;
            }
            i += 1;
            continue;
        }

        // Regular paragraph
        out.push_str("<p class=\"doc-paragraph\">");
        out.push_str(&render_inline(trimmed));
        out.push_str("</p>\n");
        i += 1;
    }

    if in_code_block {
        out.push_str("</code></pre></div>\n");
    }
    if in_list {
        out.push_str("</ul>\n");
    }
    if in_table {
        out.push_str("</tbody></table></div>\n");
    }
    if in_blockquote {
        out.push_str("</div></div>\n");
    }

    out
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

    #[test]
    fn test_all_pages_exist() {
        assert_eq!(all_pages().len(), 6);
        assert!(get_page("overview").is_some());
        assert!(get_page("quickstart").is_some());
        assert!(get_page("profiles").is_some());
        assert!(get_page("api").is_some());
        assert!(get_page("overrides").is_some());
        assert!(get_page("architecture").is_some());
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
}
