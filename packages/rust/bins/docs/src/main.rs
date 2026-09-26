use topcoat::Error;
use topcoat::router::{Router, internal_serve, page};
use topcoat::view::{View, view};

const CSS: &str = r#"
    :root {
      --bg: #090d16;
      --card-bg: #111827;
      --card-border: #1f293d;
      --card-hover: #1e293b;
      --text: #f1f5f9;
      --text-muted: #94a3b8;
      --accent: #6366f1;
      --accent-glow: rgba(99, 102, 241, 0.25);
      --accent-secondary: #06b6d4;
      --emerald: #10b981;
      --emerald-glow: rgba(16, 185, 129, 0.2);
      --amber: #f59e0b;
      --code-bg: #030712;
      --sidebar-width: 260px;
    }
    * { box-sizing: border-box; margin: 0; padding: 0; }
    body {
      font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
      background: var(--bg);
      color: var(--text);
      line-height: 1.6;
      display: flex;
      min-height: 100vh;
    }
    aside {
      width: var(--sidebar-width);
      background: #0b0f19;
      border-right: 1px solid var(--card-border);
      padding: 2rem 1.5rem;
      position: fixed;
      height: 100vh;
      overflow-y: auto;
      display: flex;
      flex-direction: column;
      gap: 1.5rem;
    }
    .brand {
      display: flex;
      align-items: center;
      gap: 0.75rem;
      font-size: 1.35rem;
      font-weight: 700;
      color: #fff;
      text-decoration: none;
    }
    .brand-badge {
      background: linear-gradient(135deg, var(--accent), var(--accent-secondary));
      padding: 0.2rem 0.5rem;
      border-radius: 6px;
      font-size: 0.75rem;
      font-weight: 600;
      color: #fff;
    }
    nav ul { list-style: none; display: flex; flex-direction: column; gap: 0.35rem; }
    nav a {
      color: var(--text-muted);
      text-decoration: none;
      padding: 0.5rem 0.75rem;
      border-radius: 6px;
      font-size: 0.9rem;
      font-weight: 500;
      display: flex;
      align-items: center;
      transition: all 0.15s ease;
    }
    nav a:hover {
      color: #fff;
      background: var(--card-hover);
    }
    main {
      margin-left: var(--sidebar-width);
      flex: 1;
      padding: 3rem 4rem 6rem;
      max-width: 1100px;
    }
    .hero {
      margin-bottom: 3.5rem;
      padding-bottom: 2rem;
      border-bottom: 1px solid var(--card-border);
    }
    .hero h1 {
      font-size: 2.8rem;
      font-weight: 800;
      letter-spacing: -0.03em;
      margin-bottom: 0.75rem;
      background: linear-gradient(135deg, #ffffff 0%, #cbd5e1 50%, var(--accent) 100%);
      -webkit-background-clip: text;
      -webkit-text-fill-color: transparent;
    }
    .hero p {
      font-size: 1.15rem;
      color: var(--text-muted);
      max-width: 720px;
      line-height: 1.6;
    }
    .badge-bar {
      display: flex;
      gap: 0.75rem;
      margin-top: 1.25rem;
      flex-wrap: wrap;
    }
    .pill {
      display: inline-flex;
      align-items: center;
      padding: 0.3rem 0.7rem;
      border-radius: 9999px;
      font-size: 0.8rem;
      font-weight: 500;
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      color: var(--text-muted);
    }
    .pill.emerald { color: var(--emerald); border-color: rgba(16, 185, 129, 0.4); background: var(--emerald-glow); }
    .pill.accent { color: var(--accent); border-color: rgba(99, 102, 241, 0.4); background: var(--accent-glow); }
    .grid {
      display: grid;
      grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
      gap: 1.25rem;
      margin: 1.5rem 0;
    }
    .card {
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 10px;
      padding: 1.5rem;
    }
    .card h3 {
      font-size: 1.1rem;
      font-weight: 600;
      margin-bottom: 0.5rem;
      color: #fff;
    }
    .card p {
      font-size: 0.9rem;
      color: var(--text-muted);
      line-height: 1.5;
    }
    pre {
      background: var(--code-bg);
      border: 1px solid var(--card-border);
      border-radius: 8px;
      padding: 1.2rem;
      overflow-x: auto;
      font-family: monospace;
      font-size: 0.875rem;
      color: #e2e8f0;
      margin: 1.25rem 0;
    }
    code {
      font-family: monospace;
      font-size: 0.875rem;
      background: rgba(255, 255, 255, 0.08);
      padding: 0.15rem 0.35rem;
      border-radius: 4px;
      color: #38bdf8;
    }
    pre code { background: transparent; padding: 0; color: inherit; }
    section { margin-bottom: 3.5rem; }
    section h2 {
      font-size: 1.75rem;
      font-weight: 700;
      margin-bottom: 1rem;
      color: #fff;
      border-bottom: 1px solid var(--card-border);
      padding-bottom: 0.5rem;
    }
    table {
      width: 100%;
      border-collapse: collapse;
      margin: 1.5rem 0;
      background: var(--card-bg);
      border: 1px solid var(--card-border);
      border-radius: 8px;
      overflow: hidden;
    }
    th, td {
      padding: 0.75rem 1rem;
      text-align: left;
      border-bottom: 1px solid var(--card-border);
      font-size: 0.875rem;
    }
    th { background: #0f172a; font-weight: 600; color: #e2e8f0; }
    tr:last-child td { border-bottom: none; }
"#;

#[page("/")]
async fn home() -> Result<impl View, Error> {
    Ok(view! {
        <html>
            <head>
                <title>"bloomery — Pure Nix Rust Builder"</title>
                <style>(CSS)</style>
            </head>
            <body>
                <aside>
                    <a href="/" class="brand">
                        <span>"bloomery"</span>
                        <span class="brand-badge">"v0.1.0"</span>
                    </a>
                    <nav>
                        <ul>
                            <li><a href="#overview">"Overview & Philosophy"</a></li>
                            <li><a href="#quickstart">"Quickstart"</a></li>
                            <li><a href="#outputs">"Outputs & Isolated Flakes"</a></li>
                            <li><a href="/profiles">"Strongly-Typed Profiles"</a></li>
                            <li><a href="/api">"mkWorkspace API Reference"</a></li>
                        </ul>
                    </nav>
                </aside>
                <main>
                    <div class="hero">
                        <h1>"bloomery"</h1>
                        <p>"A pure, fast Nix builder for Rust applications and workspaces directly from Cargo.lock — dogfooding itself by compiling this Topcoat web application."</p>
                        <div class="badge-bar">
                            <span class="pill emerald">"✓ Zero Cargo Overhead"</span>
                            <span class="pill accent">"⚡ Pure Nix Derivations"</span>
                            <span class="pill">"🔒 Hermetic & Reproducible"</span>
                            <span class="pill">"🦀 Built with Topcoat"</span>
                        </div>
                    </div>

                    <section id="overview">
                        <h2>"Overview & Philosophy"</h2>
                        <div class="grid">
                            <div class="card">
                                <h3>"📦 True Derivation Per Crate"</h3>
                                <p>"Every crate in Cargo.lock becomes an independent derivation. Editing one file rebuilds only that binary in seconds; dependencies remain fully cached in /nix/store."</p>
                            </div>
                            <div class="card">
                                <h3>"⚡ Instant Parallel Checks"</h3>
                                <p>"Check targets (test, clippy, doc, doctest, bin, lib) execute independently with zero redundant recompilations."</p>
                            </div>
                            <div class="card">
                                <h3>"🧪 Isolated Test Flakes"</h3>
                                <p>"Test workspaces are self-contained flakes with isolated checks, keeping the main flake output listing clean and focused."</p>
                            </div>
                        </div>
                    </section>

                    <section id="quickstart">
                        <h2>"Quickstart"</h2>
                        <p>"Import bloomery in your flake.nix and call mkWorkspace:"</p>
                        <pre><code>{r#"{
  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
    bloomery.url = "path:./bloomery";
  };

  outputs = { self, nixpkgs, bloomery }:
    let
      system = "x86_64-linux";
      pkgs = nixpkgs.legacyPackages.${system};
      bl = bloomery.mkLib.${system};
      workspace = bl.mkWorkspace {
        root = ./.;
        profile = {
          optLevel = 3;
          lto = "fat";
          codegenUnits = 1;
        };
      };
    in {
      packages.${system} = workspace.packages;
      apps.${system} = workspace.apps;
      checks.${system} = workspace.checks;
    };
}"#}</code></pre>
                    </section>

                    <section id="outputs">
                        <h2>"Outputs & Isolated Flakes"</h2>
                        <table>
                            <thead>
                                <tr>
                                    <th>"Flake Target"</th>
                                    <th>"Outputs"</th>
                                    <th>"Description"</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr>
                                    <td><code>"bloomery (main)"</code></td>
                                    <td><code>"packages.default, apps.default, checks.unit-tests"</code></td>
                                    <td>"Exports the library, unit-tests, and serves this Topcoat documentation server."</td>
                                </tr>
                                <tr>
                                    <td><code>"tests/basic-workspace"</code></td>
                                    <td><code>"checks.* (24 CI checks), packages.*"</code></td>
                                    <td>"Isolated sub-flake running all integration checks without polluting the main flake."</td>
                                </tr>
                                <tr>
                                    <td><code>"tests/axum-workspace"</code></td>
                                    <td><code>"checks.* (8 CI checks), packages.*"</code></td>
                                    <td>"Async web service test workspace with Axum server and Clap CLI client."</td>
                                </tr>
                            </tbody>
                        </table>
                    </section>
                </main>
            </body>
        </html>
    })
}

#[page("/profiles")]
async fn profiles() -> Result<impl View, Error> {
    Ok(view! {
        <html>
            <head>
                <title>"bloomery — Strongly-Typed Profiles"</title>
                <style>(CSS)</style>
            </head>
            <body>
                <aside>
                    <a href="/" class="brand">
                        <span>"bloomery"</span>
                        <span class="brand-badge">"Profiles"</span>
                    </a>
                    <nav>
                        <ul>
                            <li><a href="/">"← Back to Overview"</a></li>
                            <li><a href="/api">"mkWorkspace API Reference"</a></li>
                        </ul>
                    </nav>
                </aside>
                <main>
                    <section>
                        <h2>"Strongly-Typed Profile System"</h2>
                        <p>"Binary profiles are validated at evaluation time by Nix modules, converting options into exact rustc flags:"</p>
                        <table>
                            <thead>
                                <tr>
                                    <th>"Option"</th>
                                    <th>"Type"</th>
                                    <th>"Default"</th>
                                    <th>"Generated rustc Flag"</th>
                                </tr>
                            </thead>
                            <tbody>
                                <tr>
                                    <td><code>"optLevel"</code></td>
                                    <td><code>"0, 1, 2, 3, s, z"</code></td>
                                    <td><code>"3"</code></td>
                                    <td><code>"-Copt-level=3"</code></td>
                                </tr>
                                <tr>
                                    <td><code>"lto"</code></td>
                                    <td><code>"fat, thin, off, full, none, bool"</code></td>
                                    <td><code>"null"</code></td>
                                    <td><code>"-Clto=fat"</code></td>
                                </tr>
                                <tr>
                                    <td><code>"codegenUnits"</code></td>
                                    <td><code>"positive integer"</code></td>
                                    <td><code>"null"</code></td>
                                    <td><code>"-Ccodegen-units=1"</code></td>
                                </tr>
                                <tr>
                                    <td><code>"panic"</code></td>
                                    <td><code>"unwind, abort"</code></td>
                                    <td><code>"null"</code></td>
                                    <td><code>"-Cpanic=abort"</code></td>
                                </tr>
                                <tr>
                                    <td><code>"strip"</code></td>
                                    <td><code>"none, debuginfo, symbols, bool"</code></td>
                                    <td><code>"null"</code></td>
                                    <td><code>"-Cstrip=symbols"</code></td>
                                </tr>
                                <tr>
                                    <td><code>"targetCpu"</code></td>
                                    <td><code>"string"</code></td>
                                    <td><code>"null"</code></td>
                                    <td><code>"-Ctarget-cpu=native"</code></td>
                                </tr>
                            </tbody>
                        </table>
                    </section>
                </main>
            </body>
        </html>
    })
}

#[page("/api")]
async fn api() -> Result<impl View, Error> {
    Ok(view! {
        <html>
            <head>
                <title>"bloomery — mkWorkspace API"</title>
                <style>(CSS)</style>
            </head>
            <body>
                <aside>
                    <a href="/" class="brand">
                        <span>"bloomery"</span>
                        <span class="brand-badge">"API"</span>
                    </a>
                    <nav>
                        <ul>
                            <li><a href="/">"← Back to Overview"</a></li>
                            <li><a href="/profiles">"Strongly-Typed Profiles"</a></li>
                        </ul>
                    </nav>
                </aside>
                <main>
                    <section>
                        <h2>"mkWorkspace API Reference"</h2>
                        <pre><code>{r#"bl.mkWorkspace {
  root = ./.;                                  # Workspace root path
  cargoLock ? root + "/Cargo.lock";           # Path to Cargo.lock
  cargoToml ? root + "/Cargo.toml";           # Path to Cargo.toml
  rustc ? pkgs.rustc;                         # Rust compiler
  clippy ? pkgs.clippy;                       # Clippy driver
  unifyFeatures ? true;                       # Unify features across workspace
  profile ? {                                 # Strongly-typed compilation profile
    optLevel = 3;
    lto = "fat";
    codegenUnits = 1;
    panic = "abort";
    strip = true;
  };
  overrides ? {};                             # Crate build overrides & features
}"#}</code></pre>
                    </section>
                </main>
            </body>
        </html>
    })
}

#[tokio::main]
async fn main() {
    let port: u16 = std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8080);
    println!("Starting bloomery docs server on http://127.0.0.1:{}", port);
    let router = Router::builder()
        .page(home)
        .page(profiles)
        .page(api)
        .build();
    let listener = tokio::net::TcpListener::bind(("127.0.0.1", port))
        .await
        .expect("bind port");
    internal_serve(listener, router.into(), std::future::pending())
        .await
        .expect("serve");
}
