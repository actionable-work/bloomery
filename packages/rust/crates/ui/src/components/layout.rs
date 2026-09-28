use topcoat::{
    Result,
    view::{Child, View, component, view},
};

use crate::assets::{BLOOMERY_CSS, BLOOMERY_FAVICON, BLOOMERY_LOGO};
use crate::components::nav::{sidebar, toc_sidebar};

#[component]
pub async fn page_layout(
    title: &str,
    active_path: &str,
    #[default] child: Child<'_>,
) -> Result<impl View> {
    let full_title = if title.is_empty() {
        "bloomery — Pure Nix Rust Engine".to_string()
    } else {
        format!("{title} — bloomery")
    };

    Ok(view! {
        <html class="dark" lang="en">
            <head>
                <meta charset="utf-8" />
                <meta name="viewport" content="width=device-width, initial-scale=1.0" />
                <title>(full_title)</title>
                <link rel="icon" type="image/svg+xml" href=(BLOOMERY_FAVICON) />
                <link rel="apple-touch-icon" href=(BLOOMERY_FAVICON) />
                <link rel="preconnect" href="https://fonts.googleapis.com" />
                <link rel="preconnect" href="https://fonts.gstatic.com" crossorigin="anonymous" />
                <link href="https://fonts.googleapis.com/css2?family=Inter:wght@400;500;600;700;800&family=JetBrains+Mono:wght@400;500;600&display=swap" rel="stylesheet" />
                <link href="https://fonts.googleapis.com/css2?family=Material+Symbols+Outlined:opsz,wght,FILL,GRAD@20..48,100..700,0..1,-50..200" rel="stylesheet" />
                <link rel="stylesheet" href=(BLOOMERY_CSS) />
            </head>
            <body class="bloomery-shell">
                <header class="bloomery-top-header">
                    <div class="header-left">
                        <a href="/" class="brand-link">
                            <img src=(BLOOMERY_LOGO) class="brand-logo-img" alt="Bloomery logo" />
                            <span class="brand-text">"bloomery"</span>
                        </a>
                        <span class="brand-version-badge">"v0.1.0"</span>
                    </div>

                    <div class="header-center">
                        <div class="quick-search-box">
                            <span class="material-symbols-outlined" style="font-size: 18px;">"search"</span>
                            <span>"Quick search documentation..."</span>
                            <kbd class="search-kbd">"⌘K"</kbd>
                        </div>
                    </div>

                    <div class="header-right">
                        <nav class="header-nav">
                            <a href="/" class=(if active_path == "/" { "header-nav-link active" } else { "header-nav-link" })>"Docs"</a>
                            <a href="/docs/quickstart" class=(if active_path == "/docs/quickstart" { "header-nav-link active" } else { "header-nav-link" })>"Guides"</a>
                            <a href="/api" class=(if active_path == "/api" { "header-nav-link active" } else { "header-nav-link" })>"API Reference"</a>
                            <a href="/profiles" class=(if active_path == "/profiles" { "header-nav-link active" } else { "header-nav-link" })>"Profiles"</a>
                            <a href="/docs/architecture" class=(if active_path == "/docs/architecture" { "header-nav-link active" } else { "header-nav-link" })>"Architecture"</a>
                        </nav>
                        <div class="header-actions">
                            <a href="https://github.com/actionable/bloomery" target="_blank" rel="noopener noreferrer" class="header-action-btn" title="GitHub Repository">
                                <span class="material-symbols-outlined" style="font-size: 16px; color: var(--primary);">"code"</span>
                                <span>"GitHub"</span>
                            </a>
                            <a href="https://github.com/actionable/bloomery/discussions" target="_blank" rel="noopener noreferrer" class="header-action-btn" title="Discussions">
                                <span class="material-symbols-outlined" style="font-size: 18px;">"forum"</span>
                            </a>
                        </div>
                    </div>
                </header>

                <div class="bloomery-main-container">
                    sidebar(active_path: active_path)
                    toc_sidebar(active_path: active_path)
                    <main class="bloomery-content">
                        (child)
                    </main>
                </div>

                <div id="search-modal" class="search-modal-backdrop" style="display: none;">
                    <div class="search-modal-card">
                        <div class="search-input-row">
                            <span class="material-symbols-outlined" style="color: var(--primary); font-size: 20px;">"search"</span>
                            <input type="text" id="docs-search-input" class="search-modal-input" placeholder="Search documentation, guides, and options..." autocomplete="off" />
                            <kbd class="search-kbd" id="close-search-btn" style="cursor: pointer;">"ESC"</kbd>
                        </div>
                        <div class="search-results-list" id="search-results">
                            <a href="/" class="search-result-item">
                                <span class="material-symbols-outlined result-icon">"home"</span>
                                <div>
                                    <div class="result-title">"Introduction & Philosophy"</div>
                                    <div class="result-desc">"Pure Nix Rust engine without Cargo lock contention"</div>
                                </div>
                            </a>
                            <a href="/docs/quickstart" class="search-result-item">
                                <span class="material-symbols-outlined result-icon">"rocket_launch"</span>
                                <div>
                                    <div class="result-title">"Quickstart Guide"</div>
                                    <div class="result-desc">"Initialize flake.nix and compile in under 60 seconds"</div>
                                </div>
                            </a>
                            <a href="/profiles" class="search-result-item">
                                <span class="material-symbols-outlined result-icon">"tune"</span>
                                <div>
                                    <div class="result-title">"Strongly-Typed Profiles"</div>
                                    <div class="result-desc">"LTO, codegen units, panic strategies, and target-cpu"</div>
                                </div>
                            </a>
                            <a href="/api" class="search-result-item">
                                <span class="material-symbols-outlined result-icon">"menu_book"</span>
                                <div>
                                    <div class="result-title">"mkWorkspace Options API"</div>
                                    <div class="result-desc">"Categorized options reference for sources, toolchain, flags"</div>
                                </div>
                            </a>
                            <a href="/overrides" class="search-result-item">
                                <span class="material-symbols-outlined result-icon">"settings_input_component"</span>
                                <div>
                                    <div class="result-title">"Colocated Overrides"</div>
                                    <div class="result-desc">"overrides.nix, native C libraries, and asset pipelines"</div>
                                </div>
                            </a>
                            <a href="/docs/architecture" class="search-result-item">
                                <span class="material-symbols-outlined result-icon">"account_tree"</span>
                                <div>
                                    <div class="result-title">"Architecture & Internals"</div>
                                    <div class="result-desc">"Zero-IFD resolution, per-crate DAGs, and build scripts"</div>
                                </div>
                            </a>
                        </div>
                    </div>
                </div>

                <script>
                    "// Tab switching interactivity
                    document.addEventListener('click', function(e) {
                        var tab = e.target.closest('.tab-btn');
                        if (!tab) return;
                        var group = tab.closest('.tab-btn-group');
                        if (!group) return;
                        group.querySelectorAll('.tab-btn').forEach(function(b) { b.classList.remove('active'); });
                        tab.classList.add('active');
                        var tabName = tab.getAttribute('data-tab');
                        var container = tab.closest('.code-window');
                        if (container && tabName) {
                            var pane = container.querySelector('.code-block');
                            if (tabName === 'flake') {
                                pane.innerHTML = '<span class=\"syntax-punct\">{</span>\\n  <span class=\"syntax-kw\">inputs</span> = <span class=\"syntax-punct\">{</span>\\n    <span class=\"syntax-fn\">nixpkgs</span>.url = <span class=\"syntax-str\">\"github:NixOS/nixpkgs/nixos-unstable\"</span>;\\n    <span class=\"syntax-fn\">bloomery</span>.url = <span class=\"syntax-str\">\"github:actionable/bloomery\"</span>;\\n  <span class=\"syntax-punct\">}</span>;\\n\\n  <span class=\"syntax-kw\">outputs</span> = <span class=\"syntax-punct\">{</span> nixpkgs, bloomery, ... <span class=\"syntax-punct\">}</span>:\\n    bloomery.mkFlake <span class=\"syntax-punct\">{</span>\\n      <span class=\"syntax-kw\">inherit</span> nixpkgs;\\n      root = ./.;\\n    <span class=\"syntax-punct\">}</span>;\\n<span class=\"syntax-punct\">}</span>';
                            } else if (tabName === 'overrides') {
                                pane.innerHTML = '<span class=\"syntax-comment\"># overrides.nix colocated next to Cargo.toml</span>\\n<span class=\"syntax-punct\">{</span> pkgs, lib <span class=\"syntax-punct\">}</span>:\\n<span class=\"syntax-punct\">{</span>\\n  fileset = ./.;\\n  rustcFlags = [ <span class=\"syntax-str\">\"-C\"</span> <span class=\"syntax-str\">\"opt-level=3\"</span> ];\\n  env = <span class=\"syntax-punct\">{</span>\\n    MY_VAR = <span class=\"syntax-str\">\"custom_value\"</span>;\\n  <span class=\"syntax-punct\">}</span>;\\n<span class=\"syntax-punct\">}</span>';
                            } else if (tabName === 'devshell') {
                                pane.innerHTML = '<span class=\"syntax-comment\"># Automatically exposed in devShells.default</span>\\n$ nix develop\\n\\n<span class=\"syntax-comment\"># Drops you into an isolated shell with rustc, cargo, rust-analyzer, clippy</span>';
                            }
                        }
                    });

                    // Search modal interactions
                    function openSearch() {
                        var modal = document.getElementById('search-modal');
                        var input = document.getElementById('docs-search-input');
                        if (modal) {
                            modal.style.display = 'flex';
                            if (input) { input.value = ''; input.focus(); }
                            filterResults('');
                        }
                    }

                    function closeSearch() {
                        var modal = document.getElementById('search-modal');
                        if (modal) { modal.style.display = 'none'; }
                    }

                    function filterResults(query) {
                        var q = query.toLowerCase().trim();
                        var items = document.querySelectorAll('.search-result-item');
                        items.forEach(function(item) {
                            var title = (item.querySelector('.result-title') || {}).textContent || '';
                            var desc = (item.querySelector('.result-desc') || {}).textContent || '';
                            var match = !q || title.toLowerCase().includes(q) || desc.toLowerCase().includes(q);
                            item.style.display = match ? 'flex' : 'none';
                        });
                    }

                    document.addEventListener('click', function(e) {
                        if (e.target.closest('.quick-search-box')) {
                            openSearch();
                            return;
                        }
                        if (e.target.id === 'search-modal' || e.target.id === 'close-search-btn') {
                            closeSearch();
                            return;
                        }
                    });

                    document.addEventListener('keydown', function(e) {
                        if ((e.metaKey || e.ctrlKey) && e.key === 'k') {
                            e.preventDefault();
                            var modal = document.getElementById('search-modal');
                            if (modal && modal.style.display === 'flex') { closeSearch(); } else { openSearch(); }
                        } else if (e.key === 'Escape') {
                            closeSearch();
                        }
                    });

                    var searchInput = document.getElementById('docs-search-input');
                    if (searchInput) {
                        searchInput.addEventListener('input', function(e) {
                            filterResults(e.target.value);
                        });
                    }

                    // Quick copy interaction
                    document.addEventListener('click', function(e) {
                        var copyBtn = e.target.closest('#copy-code-btn') || e.target.closest('#copy-nix-eval');
                        if (copyBtn) {
                            var cmd = copyBtn.getAttribute('data-copy') || 'nix flake check';
                            navigator.clipboard.writeText(cmd).then(function() {
                                var icon = copyBtn.querySelector('.material-symbols-outlined');
                                if (icon) {
                                    var orig = icon.textContent;
                                    icon.textContent = 'check';
                                    setTimeout(function() { icon.textContent = orig; }, 1500);
                                }
                            });
                        }
                        var fYes = e.target.closest('#feedback-yes');
                        var fNo = e.target.closest('#feedback-no');
                        if (fYes) {
                            fYes.style.background = 'rgba(52, 211, 153, 0.2)';
                            fYes.style.color = '#34d399';
                        }
                        if (fNo) {
                            fNo.style.background = 'rgba(255, 180, 171, 0.2)';
                            fNo.style.color = '#ffb4ab';
                        }
                    });"
                </script>
            </body>
        </html>
    })
}
