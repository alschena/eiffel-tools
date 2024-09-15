use async_lsp::concurrency::ConcurrencyLayer;
use async_lsp::lsp_types::request;
use async_lsp::panic::CatchUnwindLayer;
use async_lsp::router;
use async_lsp::server::LifecycleLayer;
use async_lsp::tracing::TracingLayer;
use async_lsp::{client_monitor::ClientProcessMonitorLayer, lsp_types::notification};
use eiffel_tools::lib::language_server_protocol::common::{Router, TickEvent};
use std::time::Duration;
use tower::ServiceBuilder;
use tracing::{info, Level};

#[tokio::main(flavor = "current_thread")]
async fn main() {
    let (server, _) = async_lsp::MainLoop::new_server(|client| {
        tokio::spawn({
            let client = client.clone();
            async move {
                let mut interval = tokio::time::interval(Duration::from_secs(1));
                loop {
                    interval.tick().await;
                    if client.emit(TickEvent).is_err() {
                        break;
                    }
                }
            }
        });

        let mut router = Router::new(&client);
        router.set_handler_request::<request::Initialize>();
        router.set_handler_request::<request::HoverRequest>();
        router.set_handler_request::<request::GotoDefinition>();
        router.set_handler_request::<request::DocumentSymbolRequest>();
        router.set_handler_request::<request::WorkspaceSymbolRequest>();
        router.set_handler_notification::<notification::Initialized>();
        router.set_handler_notification::<notification::DidOpenTextDocument>();
        router.set_handler_notification::<notification::DidChangeTextDocument>();
        router.set_handler_notification::<notification::DidCloseTextDocument>();
        router.set_tick_event();

        ServiceBuilder::new()
            .layer(TracingLayer::default())
            .layer(LifecycleLayer::default())
            .layer(CatchUnwindLayer::default())
            .layer(ConcurrencyLayer::default())
            .layer(ClientProcessMonitorLayer::new(client))
            .service::<router::Router<_>>(router.into())
    });

    tracing_subscriber::fmt()
        .with_max_level(Level::INFO)
        .with_ansi(false)
        .with_writer(std::io::stderr)
        .init();

    // Prefer truly asynchronous piped stdin/stdout without blocking tasks.
    #[cfg(unix)]
    let (stdin, stdout) = (
        async_lsp::stdio::PipeStdin::lock_tokio().unwrap(),
        async_lsp::stdio::PipeStdout::lock_tokio().unwrap(),
    );
    // Fallback to spawn blocking read/write otherwise.
    #[cfg(not(unix))]
    let (stdin, stdout) = (
        tokio_util::compat::TokioAsyncReadCompatExt::compat(tokio::io::stdin()),
        tokio_util::compat::TokioAsyncWriteCompatExt::compat_write(tokio::io::stdout()),
    );

    server.run_buffered(stdin, stdout).await.unwrap();
}
