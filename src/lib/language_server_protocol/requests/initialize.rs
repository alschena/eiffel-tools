use crate::lib::language_server_protocol::prelude::*;
use async_lsp::lsp_types::{
    request::{Initialize, Request},
    HoverProviderCapability, InitializeResult, OneOf, ServerCapabilities,
};
use async_lsp::ResponseError;
use std::future::Future;
use std::path::PathBuf;

impl HandleRequest for Initialize {
    fn handle_request(
        st: ServerState,
        params: <Self as Request>::Params,
    ) -> impl Future<Output = Result<<Self as Request>::Result, ResponseError>> + Send + 'static
    {
        params.initialization_options.map(|x| {
            x.as_object().map(|z| {
                z.get("ecf_path").map(|path| {
                    let mut workspace =
                        st.workspace.write().expect("workspace must be writable");
                    workspace.set_ecf_path(Some(
                        PathBuf::try_from(path.as_str().expect("must be string"))
                            .expect("must be a valid path"),
                    ));
                });
            })
        });
        async move {
            Ok(InitializeResult {
                capabilities: ServerCapabilities {
                    hover_provider: Some(HoverProviderCapability::Simple(true)),
                    definition_provider: Some(OneOf::Left(true)),
                    document_symbol_provider: Some(OneOf::Left(true)),
                    workspace_symbol_provider: Some(OneOf::Left(true)),
                    code_action_provider: Some(true.into()),
                    ..ServerCapabilities::default()
                },
                server_info: None,
            })
        }
    }
}
