use super::super::prelude::{HandleRequest, ServerState};
use async_lsp::lsp_types::{
    notification, request, Hover, HoverContents, MarkedString, MessageType, ShowMessageParams,
};
use async_lsp::ResponseError;
use async_lsp::Result;
use std::future::Future;
use std::time::Duration;
impl HandleRequest for request::HoverRequest {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        let client = st.client.clone();
        async move {
            tokio::time::sleep(Duration::from_secs(1)).await;
            client
                .notify::<notification::ShowMessage>(ShowMessageParams {
                    typ: MessageType::INFO,
                    message: "Hello LSP".into(),
                })
                .unwrap();
            Ok(Some(Hover {
                contents: HoverContents::Scalar(MarkedString::String(format!("I am a hover text"))),
                range: None,
            }))
        }
    }
}
