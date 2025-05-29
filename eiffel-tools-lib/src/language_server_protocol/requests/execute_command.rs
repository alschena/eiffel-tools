use super::HandleRequest;
use async_lsp::lsp_types::request;

use crate::language_server_protocol::commands::Commands;

impl HandleRequest for request::ExecuteCommand {
    async fn handle_request(
        st: super::ServerState,
        params: <Self as request::Request>::Params,
    ) -> async_lsp::Result<<Self as request::Request>::Result, async_lsp::ResponseError> {
        let ws = st.workspace.read().await;
        let client = st.client;
        let generators = st.generators.read().await;
        let mut command = Commands::try_new(&ws, params).map_err(|e| {
            async_lsp::ResponseError::new(
                async_lsp::ErrorCode::INVALID_REQUEST,
                format!("fais to generate command from workspace and parameter with error: {e}"),
            )
        })?;
        command.run(&client, &generators).await?;
        Ok(None)
    }
}
