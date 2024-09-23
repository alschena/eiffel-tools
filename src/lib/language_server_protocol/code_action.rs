use super::common::{HandleRequest, ServerState};
use async_lsp::lsp_types::{
    request, CodeAction, CodeActionDisabled, CodeActionOrCommand, CodeActionResponse, Command,
    SymbolInformation,
};
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
                .expect("Code action on not yet parsed file");
            let range = params.range.into();
            let surrounding_feature = processed_file.feature_around(range);

            let title = "Add contracts to current routine".into();
            let kind = None;
            let diagnostics = None;
            let command = None;
            let is_preferred = Some(false);
            let data = None;
            let mut response = CodeActionResponse::new();
            match surrounding_feature {
                Some(f) => {
                    let edit = None; // TODO
                    let disabled = None;
                    let code_action = CodeAction {
                        title,
                        kind,
                        diagnostics,
                        edit,
                        command,
                        is_preferred,
                        disabled,
                        data,
                    };
                    // let command = Command::new(title, "add_contracts_routine".to_string(), None);
                    response.push(CodeActionOrCommand::CodeAction(code_action));
                    // response.push(CodeActionOrCommand::Command(command));
                    Ok(Some(response))
                }
                None => {
                    let disabled = Some(CodeActionDisabled {
                        reason: "The cursor is not surrounded by a feature".to_string(),
                    });
                    let edit = None;
                    let code_action = CodeAction {
                        title,
                        kind,
                        diagnostics,
                        edit,
                        command,
                        is_preferred,
                        disabled,
                        data,
                    };
                    response.push(CodeActionOrCommand::CodeAction(code_action));
                    // response.push(CodeActionOrCommand::Command(command));
                    Ok(Some(response))
                }
            }
        }
    }
}

#[cfg(test)]
mod test {}
