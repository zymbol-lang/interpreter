//! Zymbol-Lang Language Server — standalone binary entry point.
//!
//! All server logic lives in `lib.rs`; this file only initialises tracing
//! and delegates to `zymbol_lsp::run()`.

use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::from_default_env()
                .add_directive("zymbol_lsp=debug".parse().unwrap())
                .add_directive("tower_lsp=info".parse().unwrap()),
        )
        .with_writer(std::io::stderr)
        .init();

    zymbol_lsp::run().await;
}
