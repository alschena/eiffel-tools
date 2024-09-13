use crate::lib::code_entities::*;
use crate::lib::processed_file::ProcessedFile;
use async_lsp::lsp_types::{
    notification, request, DocumentSymbol, DocumentSymbolParams, DocumentSymbolResponse, Hover,
    HoverContents, HoverProviderCapability, InitializeResult, MarkedString, MessageType, OneOf,
    ServerCapabilities, ShowMessageParams, SymbolKind, Url, WorkspaceLocation, WorkspaceSymbol,
    WorkspaceSymbolResponse,
};
use async_lsp::router;
use async_lsp::ClientSocket;
use async_lsp::{lsp_types, ResponseError};
use async_lsp::{Error, Result};
use std::future::Future;
use std::ops::ControlFlow;
use std::path;
use std::string::ParseError;
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tracing::{info, Level};
use tree_sitter::Parser;

impl From<DocumentSymbol> for Feature<'_> {
    fn from(value: DocumentSymbol) -> Self {
        let name = value.name;
        let kind = value.kind;
        let range = value.range;
        debug_assert_ne!(kind, SymbolKind::CLASS);
        Feature::from_name_and_range(name, range.into())
    }
}

impl From<&Feature<'_>> for DocumentSymbol {
    fn from(value: &Feature<'_>) -> Self {
        let name = value.name().to_string();
        let range = value.range().clone().into();
        DocumentSymbol {
            name,
            detail: None,
            kind: SymbolKind::METHOD,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children: None,
        }
    }
}

impl From<DocumentSymbol> for Class<'_> {
    fn from(value: DocumentSymbol) -> Self {
        let name = value.name;
        let kind = value.kind;
        let range = value.range;
        debug_assert_eq!(kind, SymbolKind::CLASS);
        let children: Vec<Feature> = match value.children {
            Some(v) => v.into_iter().map(|x| x.into()).collect(),
            None => Vec::new(),
        };
        Class::from_name_range(name, range.into())
    }
}

impl From<&Class<'_>> for DocumentSymbol {
    fn from(value: &Class<'_>) -> Self {
        let name = value.name().to_string();
        let features = value.features();
        let range = value.range().clone().into();
        let children: Option<Vec<DocumentSymbol>> =
            Some(features.into_iter().map(|x| x.into()).collect());
        DocumentSymbol {
            name,
            detail: None,
            kind: SymbolKind::CLASS,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children,
        }
    }
}

impl From<&Class<'_>> for WorkspaceSymbol {
    fn from(value: &Class<'_>) -> Self {
        let name = value.name().to_string();
        let features = value.features();
        let children: Option<Vec<DocumentSymbol>> =
            Some(features.into_iter().map(|x| x.into()).collect());
        let path = value
            .location()
            .expect("Expected class with valid file location");
        let location: WorkspaceLocation = path
            .try_into()
            .expect("Path cannot be converted to WorkspaceLocation");
        WorkspaceSymbol {
            name,
            kind: SymbolKind::CLASS,
            tags: None,
            container_name: None,
            location: OneOf::Right(location),
            data: None,
        }
    }
}

impl TryFrom<&Location> for WorkspaceLocation {
    type Error = ();
    fn try_from(value: &Location) -> Result<Self, ()> {
        match value.try_into() {
            Err(_) => Err(()),
            Ok(uri) => Ok(Self { uri }),
        }
    }
}

impl TryFrom<&Location> for Url {
    type Error = ();

    fn try_from(value: &Location) -> std::result::Result<Self, ()> {
        Self::from_file_path(value.path.clone())
    }
}

impl From<Point> for async_lsp::lsp_types::Position {
    fn from(value: Point) -> Self {
        Self {
            line: value.row.try_into().expect("Failed to convert row"),
            character: value.column.try_into().expect("Failed to convert column"),
        }
    }
}

impl From<async_lsp::lsp_types::Position> for Point {
    fn from(value: async_lsp::lsp_types::Position) -> Self {
        Self {
            row: value
                .line
                .try_into()
                .expect("Failed conversion of row from u32 to usize or viceversa"),
            column: value
                .character
                .try_into()
                .expect("Failed conversion of row from u32 to usize or viceversa"),
        }
    }
}

impl From<async_lsp::lsp_types::Range> for Range {
    fn from(value: async_lsp::lsp_types::Range) -> Self {
        Self {
            start: value.start.into(),
            end: value.end.into(),
        }
    }
}

impl From<Range> for async_lsp::lsp_types::Range {
    fn from(value: Range) -> Self {
        Self {
            start: value.start.into(),
            end: value.end.into(),
        }
    }
}

#[derive(Clone)]
pub struct ServerState {
    client: ClientSocket,
    workspace: Arc<RwLock<Vec<ProcessedFile>>>,
    counter: i32,
}

pub struct TickEvent;
pub trait HandleRequest: request::Request {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static;
}
pub trait HandleNotification: lsp_types::notification::Notification {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()>;
}

impl HandleRequest for request::Initialize {
    fn handle_request(
        _st: ServerState,
        params: <Self as lsp_types::request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as lsp_types::request::Request>::Result, ResponseError>>
           + Send
           + 'static {
        async move {
            eprintln!("Initialize with {params:?}");
            Ok(InitializeResult {
                capabilities: ServerCapabilities {
                    hover_provider: Some(HoverProviderCapability::Simple(true)),
                    definition_provider: Some(OneOf::Left(true)),
                    document_symbol_provider: Some(OneOf::Left(true)),
                    ..ServerCapabilities::default()
                },
                server_info: None,
            })
        }
    }
}
impl HandleNotification for notification::Initialized {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}

impl HandleRequest for request::HoverRequest {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        let client = st.client.clone();
        let counter = st.counter;
        async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            client
                .notify::<notification::ShowMessage>(ShowMessageParams {
                    typ: MessageType::INFO,
                    message: "Hello LSP".into(),
                })
                .unwrap();
            Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!(
                    "I am a hover text {counter}!"
                ))),
                range: None,
            }))
        }
    }
}

impl HandleRequest for request::GotoDefinition {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        async move { unimplemented!() }
    }
}

impl HandleRequest for request::DocumentSymbolRequest {
    fn handle_request(
        st: ServerState,
        params: DocumentSymbolParams,
    ) -> impl Future<Output = Result<Self::Result, ResponseError>> + Send + 'static {
        async move {
            let path: path::PathBuf = params.text_document.uri.path().into();
            // Read borrow
            {
                let read_workspace = st.workspace.read().unwrap();
                let file = read_workspace.iter().find(|&x| x.path == path);
                if let Some(file) = file {
                    let class: Class = file.into();
                    let symbol: DocumentSymbol = (&class).into();
                    let classes = vec![symbol];
                    return Ok(Some(DocumentSymbolResponse::Nested(classes)));
                }
            }
            // Write borrow
            {
                let mut write_workspace = st.workspace.write().unwrap();
                debug_assert!(write_workspace.iter().find(|&x| x.path == path).is_none());

                let mut parser = tree_sitter::Parser::new();
                parser
                    .set_language(tree_sitter_eiffel::language())
                    .expect("Error loading Eiffel grammar");
                let file = ProcessedFile::new(&mut parser, path);
                let class: Class = (&file).into();
                let symbol: DocumentSymbol = (&class).into();
                let classes = vec![symbol];

                write_workspace.push(file);
                return Ok(Some(DocumentSymbolResponse::Nested(classes)));
            }
        }
    }
}

impl HandleRequest for request::WorkspaceSymbolRequest {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        async move {
            let read_workspace = st.workspace.read().unwrap();

            let classes: Vec<Class<'_>> = read_workspace.iter().map(|x| x.into()).collect();
            let workspace_symbols: Vec<WorkspaceSymbol> =
                classes.iter().map(|x| x.into()).collect();
            Ok(Some(WorkspaceSymbolResponse::Nested(workspace_symbols)))
        }
    }
}

impl HandleNotification for notification::DidChangeConfiguration {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}

impl HandleNotification for notification::DidOpenTextDocument {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}

impl HandleNotification for notification::DidChangeTextDocument {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}

impl HandleNotification for notification::DidCloseTextDocument {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}

pub struct Router<T>(router::Router<T>);

impl<T> From<router::Router<T>> for Router<T> {
    fn from(value: router::Router<T>) -> Self {
        Self(value)
    }
}

impl<T> From<Router<T>> for router::Router<T> {
    fn from(value: Router<T>) -> Self {
        value.0
    }
}

impl Router<ServerState> {
    pub fn new(client: &ClientSocket) -> Router<ServerState> {
        let kernel = router::Router::new(ServerState {
            client: client.clone(),
            workspace: Arc::new(RwLock::new(Vec::new())),
            counter: 0,
        });
        Router(kernel)
    }
    pub fn set_handler_request<T: HandleRequest + 'static>(&mut self) {
        self.0
            .request::<T, _>(|st, params| T::handle_request(st.clone(), params));
    }
    pub fn set_handler_notification<T: HandleNotification + 'static>(&mut self) {
        self.0
            .notification::<T>(|st, params| T::handle_notification(st.clone(), params));
    }
    pub fn set_tick_event(&mut self) {
        self.0.event::<TickEvent>(|st, _| {
            info!("tick");
            st.counter += 1;
            ControlFlow::Continue(())
        });
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use std::fs::File;
    use std::io::prelude::*;
    use std::path::PathBuf;

    #[test]
    fn class_to_workspacesymbol() {
        let path = "/tmp/eiffel_tool_test_class_to_workspacesymbol.e";
        let path = PathBuf::from(path);
        let src = "
    class A
    note
    end
        ";
        let mut file = File::create(path.clone()).expect("Failed to create file");
        file.write_all(src.as_bytes())
            .expect("Failed to write to file");

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let file = ProcessedFile::new(&mut parser, path.clone());
        let class: Class = (&file).into();
        let location = class.location().expect("location non empty");

        eprintln!("{:?}", location);

        assert!(<WorkspaceSymbol>::try_from(&class).is_ok());
    }
}
