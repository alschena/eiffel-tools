use async_lsp::client_monitor::ClientProcessMonitorLayer;
use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::router;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use eiffel_tools::lib::language_server_protocol::prelude::*;
use std::path::Path;
use tower::ServiceBuilder;
use tracing_subscriber::filter;
use tracing_subscriber::fmt::{self, format::FmtSpan};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{Layer, Registry};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> anyhow::Result<()> {
    let (server, _) = async_lsp::MainLoop::new_server(|client| {
        let server_state = ServerState::new(client.clone());

        let mut router = Router::new(server_state);
        router.set_request_handlers();
        router.set_notification_handlers();

        ServiceBuilder::new()
            .layer(TracingLayer::default())
            .layer(LifecycleLayer::default())
            .layer(CatchUnwindLayer::default())
            .layer(ConcurrencyLayer::default())
            .layer(ClientProcessMonitorLayer::new(client))
            .service::<router::Router<_>>(router.into())
    });

    let log_directory_path = &Path::new(".lsp_eiffel.d");
    if !log_directory_path.exists() {
        std::fs::DirBuilder::new().create(log_directory_path)?;
    }

    let default_log_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(false)
        .open(log_directory_path.join("log.log"))?;

    let llm_log_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(false)
        .open(log_directory_path.join("llm.log"))?;

    let default_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false)
        .with_writer(default_log_file)
        .with_filter(
            filter::Targets::default()
                .with_default(filter::LevelFilter::INFO)
                .with_target("llm", filter::LevelFilter::OFF),
        );

    let llm_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false)
        .with_writer(llm_log_file)
        .with_filter(filter::Targets::default().with_target("llm", filter::LevelFilter::INFO));

    Registry::default()
        .with(default_layer)
        .with(llm_layer)
        .init();

    // Prefer truly asynchronous piped stdin/stdout without blocking tasks.
    #[cfg(unix)]
    let (stdin, stdout) = (
        async_lsp::stdio::PipeStdin::lock_tokio()?,
        async_lsp::stdio::PipeStdout::lock_tokio()?,
    );
    // Fallback to spawn blocking read/write otherwise.
    #[cfg(not(unix))]
    let (stdin, stdout) = (
        tokio_util::compat::TokioAsyncReadCompatExt::compat(tokio::io::stdin()),
        tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(tokio::io::stdout()),
    );

    server.run_buffered(stdin, stdout).await?;
    Ok(())
}
