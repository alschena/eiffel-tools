use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::lsp_types::request;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::router;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use async_lsp::{client_monitor::ClientProcessMonitorLayer, lsp_types::notification};
use eiffel_tools::lib::language_server_protocol::prelude::*;
use std::path::Path;
use std::time::Duration;
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

        tokio::spawn({
            let mut server = server_state.clone();
            async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    server.process_task().await
                }
            }
        });

        let mut router = Router::new(server_state);
        router.set_handler_request::<request::Initialize>();
        router.set_handler_request::<request::HoverRequest>();
        router.set_handler_request::<request::GotoDefinition>();
        router.set_handler_request::<request::DocumentSymbolRequest>();
        router.set_handler_request::<request::WorkspaceSymbolRequest>();
        router.set_handler_request::<request::CodeActionRequest>();
        router.set_handler_notification::<notification::Initialized>();
        router.set_handler_notification::<notification::DidOpenTextDocument>();
        router.set_handler_notification::<notification::DidSaveTextDocument>();
        router.set_handler_notification::<notification::DidChangeTextDocument>();
        router.set_handler_notification::<notification::DidCloseTextDocument>();
        router.set_handler_notification::<notification::DidChangeConfiguration>();

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

    let gemini_log_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .append(false)
        .open(log_directory_path.join("gemini.log"))?;

    let default_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false)
        .with_writer(default_log_file)
        .with_filter(
            filter::Targets::default()
                .with_default(filter::LevelFilter::INFO)
                .with_target("gemini", filter::LevelFilter::OFF),
        );

    let gemini_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false)
        .with_writer(gemini_log_file)
        .with_filter(filter::Targets::default().with_target("gemini", filter::LevelFilter::INFO));

    Registry::default()
        .with(default_layer)
        .with(gemini_layer)
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
