use super::common::{HandleRequest, ServerState};
use crate::lib::code_entities::*;
use async_lsp::lsp_types::{request, SymbolInformation};
use async_lsp::ResponseError;
use async_lsp::Result;
use std::future::Future;

impl HandleRequest for request::CodeActionRequest {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        async move {
            let workspace = st.workspace.read().unwrap();
            let path = params
                .text_document
                .uri
                .to_file_path()
                .expect("Path of target document of code action");
            let processed_file = workspace
                .iter()
                .find(|&x| x.path == path)
                .expect("Code action on an parsed file");
            let tree = processed_file.tree();
            let range = params.range;
            todo!()
        }
    }
}

#[cfg(test)]
mod test {}
