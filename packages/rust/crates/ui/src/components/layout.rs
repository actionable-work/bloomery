use topcoat::{
    Result,
    view::{Child, Unescaped, View, component, view},
};

use bloomery_content::CONFIG;

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
                        <span class="brand-version-badge">(CONFIG.default_version)</span>
                    </div>

                    <div class="header-center">
                        <div class="quick-search-box">
                            <span class="material-symbols-outlined" style="font-size: 18px;">"search"</span>
                            <span>"Quick search documentation..."</span>
                            <kbd class="search-kbd">"⌘K"</kbd>
                        </div>
                    </div>

                    <div class="header-right">
                        <div class="header-actions">
                            <a href=(CONFIG.repo_url) target="_blank" rel="noopener noreferrer" class="header-action-btn" title="GitHub Repository & Stars">
                                <span class="material-symbols-outlined" style="font-size: 16px; color: #f59e0b;">"star"</span>
                                <span>"GitHub"</span>
                                <span id="github-stars-count" class="github-stars-badge" style="background: rgba(245, 158, 11, 0.15); color: #f59e0b; padding: 0.15rem 0.45rem; border-radius: 4px; font-size: 0.75rem; font-weight: 700; margin-left: 0.2rem;">"★"</span>
                            </a>
                            <a href=(CONFIG.discussions_url) target="_blank" rel="noopener noreferrer" class="header-action-btn" title="Discussions">
                                <span class="material-symbols-outlined" style="font-size: 18px;">"forum"</span>
                                <span>"Discussions"</span>
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
                            <input type="text" id="docs-search-input" class="search-modal-input" placeholder="Search documentation, options, architecture..." autocomplete="off" />
                            <kbd class="search-kbd" id="close-search-btn" style="cursor: pointer;">"ESC"</kbd>
                        </div>
                        <div class="search-results-list" id="search-results">
                            <a href="/" class="search-result-item" data-keywords="overview philosophy nix cargo lock pure rlib">
                                <span class="material-symbols-outlined result-icon">"home"</span>
                                <div>
                                    <div class="result-title">"Introduction & Philosophy"</div>
                                    <div class="result-desc">"Pure Nix Rust engine, zero Cargo overhead, and core principles"</div>
                                </div>
                            </a>
                            <a href="/quickstart" class="search-result-item" data-keywords="quickstart flake mkflake mkworkspace flake-parts lock shell">
                                <span class="material-symbols-outlined result-icon">"rocket_launch"</span>
                                <div>
                                    <div class="result-title">"Quickstart & Integration Styles"</div>
                                    <div class="result-desc">"Zero-boilerplate flake.nix, flake-parts modules, and lock commands"</div>
                                </div>
                            </a>
                            <a href="/architecture" class="search-result-item" data-keywords="architecture dag zero-ifd build.rs assets rlib dependencies">
                                <span class="material-symbols-outlined result-icon">"account_tree"</span>
                                <div>
                                    <div class="result-title">"Architecture & Internals"</div>
                                    <div class="result-desc">"Zero-IFD resolution, per-crate DAGs, and build script sandboxing"</div>
                                </div>
                            </a>
                            <a href="/profiles" class="search-result-item" data-keywords="profiles lto optlevel panic codegenunits strip debuginfo targetcpu">
                                <span class="material-symbols-outlined result-icon">"tune"</span>
                                <div>
                                    <div class="result-title">"Strongly-Typed Profiles"</div>
                                    <div class="result-desc">"LTO, codegen units, panic strategies, and profileDev settings"</div>
                                </div>
                            </a>
                            <a href="/api" class="search-result-item" data-keywords="api schema options mkworkspace mkflake toolchain source flags">
                                <span class="material-symbols-outlined result-icon">"menu_book"</span>
                                <div>
                                    <div class="result-title">"mkWorkspace Options API"</div>
                                    <div class="result-desc">"Categorized options reference for sources, toolchain, flags"</div>
                                </div>
                            </a>
                            <a href="/overrides" class="search-result-item" data-keywords="overrides fileset assets assetdirs openssl sys-crates env">
                                <span class="material-symbols-outlined result-icon">"settings_input_component"</span>
                                <div>
                                    <div class="result-title">"Colocated overrides.nix"</div>
                                    <div class="result-desc">"overrides.nix, native C libraries, and asset pipelines"</div>
                                </div>
                            </a>
                            <a href="/matrix" class="search-result-item" data-keywords="matrix tests workspaces verification axum mklib edge-cases">
                                <span class="material-symbols-outlined result-icon">"fact_check"</span>
                                <div>
                                    <div class="result-title">"Verification Matrix"</div>
                                    <div class="result-desc">"6 dedicated test workspaces running automated checks in CI"</div>
                                </div>
                            </a>
                            <a href="/benchmarks" class="search-result-item" data-keywords="benchmarks comparison crane cargo2nix cache ci speed">
                                <span class="material-symbols-outlined result-icon">"query_stats"</span>
                                <div>
                                    <div class="result-title">"Architectural Benchmarks"</div>
                                    <div class="result-desc">"Performance comparisons with Crane & Cargo2nix"</div>
                                </div>
                            </a>
                            <div id="search-no-results" style="display: none; padding: 1.5rem; text-align: center; color: var(--text-muted); font-size: 0.9rem;">
                                "No matching documentation pages found."
                            </div>
                        </div>
                    </div>
                </div>

                <script>(Unescaped::new_unchecked(r#"
                    // Dynamically fetch GitHub stars
                    (function fetchGitHubStars() {
                        var badge = document.getElementById('github-stars-count');
                        if (!badge) return;
                        fetch('https://api.github.com/repos/actionable-work/bloomery')
                            .then(function(r) { return r.json(); })
                            .then(function(d) {
                                if (d && typeof d.stargazers_count === 'number') {
                                    var count = d.stargazers_count;
                                    badge.textContent = count >= 1000 ? (count / 1000).toFixed(1) + 'k' : count;
                                } else {
                                    badge.textContent = '★';
                                }
                            })
                            .catch(function() { badge.textContent = '★'; });
                    })();

                    // Tab switching interactivity
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
                                pane.innerHTML = '<span class="syntax-punct">{</span>\n  <span class="syntax-kw">inputs</span> = <span class="syntax-punct">{</span>\n    <span class="syntax-fn">nixpkgs</span>.url = <span class="syntax-str">"github:NixOS/nixpkgs/nixos-unstable"</span>;\n    <span class="syntax-fn">bloomery</span>.url = <span class="syntax-str">"github:actionable-work/bloomery"</span>;\n  <span class="syntax-punct">}</span>;\n\n  <span class="syntax-kw">outputs</span> = <span class="syntax-punct">{</span> nixpkgs, bloomery, ... <span class="syntax-punct">}</span>:\n    bloomery.mkFlake <span class="syntax-punct">{</span>\n      <span class="syntax-kw">inherit</span> nixpkgs;\n      root = ./.;\n    <span class="syntax-punct">}</span>;\n<span class="syntax-punct">}</span>';
                            } else if (tabName === 'overrides') {
                                pane.innerHTML = '<span class="syntax-comment"># overrides.nix colocated next to Cargo.toml</span>\n<span class="syntax-punct">{</span> pkgs, lib <span class="syntax-punct">}</span>:\n<span class="syntax-punct">{</span>\n  fileset = ./.;\n  rustcFlags = [ <span class="syntax-str">"-C"</span> <span class="syntax-str">"opt-level=3"</span> ];\n  env = <span class="syntax-punct">{</span>\n    MY_VAR = <span class="syntax-str">"custom_value"</span>;\n  <span class="syntax-punct">}</span>;\n<span class="syntax-punct">}</span>';
                            } else if (tabName === 'devshell') {
                                pane.innerHTML = '<span class="syntax-comment"># Automatically exposed in devShells.default</span>\n$ nix develop\n\n<span class="syntax-comment"># Drops you into an isolated shell with rustc, cargo, rust-analyzer, clippy</span>';
                            }
                        }
                    });

                    // Search modal interactions
                    function openSearch() {
                        var modal = document.getElementById('search-modal');
                        var input = document.getElementById('docs-search-input');
                        if (modal) {
                            modal.style.display = 'flex';
                            filterResults('');
                            if (input) {
                                input.value = '';
                                setTimeout(function() { input.focus(); }, 15);
                            }
                        }
                    }

                    function closeSearch() {
                        var modal = document.getElementById('search-modal');
                        if (modal) { modal.style.display = 'none'; }
                    }

                    function filterResults(query) {
                        var q = query.toLowerCase().trim();
                        var items = document.querySelectorAll('.search-result-item');
                        var count = 0;
                        items.forEach(function(item) {
                            var title = (item.querySelector('.result-title') || {}).textContent || '';
                            var desc = (item.querySelector('.result-desc') || {}).textContent || '';
                            var kw = item.getAttribute('data-keywords') || '';
                            var match = !q || title.toLowerCase().indexOf(q) !== -1 || desc.toLowerCase().indexOf(q) !== -1 || kw.toLowerCase().indexOf(q) !== -1;
                            item.style.display = match ? 'flex' : 'none';
                            if (match) count++;
                        });
                        var noRes = document.getElementById('search-no-results');
                        if (noRes) noRes.style.display = (count === 0) ? 'block' : 'none';
                    }

                    document.addEventListener('click', function(e) {
                        if (e.target.closest('.quick-search-box')) {
                            openSearch();
                            return;
                        }
                        var resultLink = e.target.closest('.search-result-item');
                        if (resultLink) {
                            closeSearch();
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
                        searchInput.addEventListener('keyup', function(e) {
                            filterResults(e.target.value);
                        });
                    }

                    // Copy code snippets and feedback box interactions
                    document.addEventListener('click', function(e) {
                        var copyBtn = e.target.closest('.copy-btn') || e.target.closest('#copy-code-btn') || e.target.closest('#copy-nix-eval');
                        if (copyBtn) {
                            var wrapper = copyBtn.closest('.code-block-wrapper');
                            var codeText = '';
                            if (wrapper) {
                                var codeEl = wrapper.querySelector('code');
                                if (codeEl) codeText = codeEl.innerText || codeEl.textContent;
                            }
                            if (!codeText) {
                                codeText = copyBtn.getAttribute('data-copy') || 'nix flake check';
                            }
                            navigator.clipboard.writeText(codeText).then(function() {
                                var origText = copyBtn.innerText || copyBtn.textContent;
                                copyBtn.innerText = 'Copied!';
                                setTimeout(function() { copyBtn.innerText = origText; }, 1800);
                            });
                        }

                        var fYes = e.target.closest('#feedback-yes');
                        var fNo = e.target.closest('#feedback-no');
                        var fContainer = e.target.closest('.feedback-actions');
                        if (fYes && fContainer) {
                            fContainer.innerHTML = '<span style="color: var(--secondary); font-weight: 600; font-size: 0.9rem; display: flex; align-items: center; gap: 0.4rem;"><span class="material-symbols-outlined" style="font-size: 18px;">check_circle</span> Thank you for your feedback!</span>';
                        }
                        if (fNo && fContainer) {
                            fContainer.innerHTML = '<span style="color: var(--primary); font-weight: 600; font-size: 0.9rem; display: flex; align-items: center; gap: 0.4rem;"><span class="material-symbols-outlined" style="font-size: 18px;">info</span> Thank you! We will work to improve this page.</span>';
                        }
                    });
                "#))</script>
            </body>
        </html>
    })
}
