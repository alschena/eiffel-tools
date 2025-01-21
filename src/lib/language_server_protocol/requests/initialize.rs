use crate::lib::config::System;
use crate::lib::language_server_protocol::prelude::*;
use async_lsp::lsp_types::{
    request::{Initialize, Request},
    HoverProviderCapability, InitializeResult, OneOf, ServerCapabilities,
    TextDocumentSyncCapability, TextDocumentSyncOptions, TextDocumentSyncSaveOptions,
};
use async_lsp::ResponseError;
use async_lsp::Result;
use std::env;
use std::fs;
use std::future::Future;
use std::path::PathBuf;

impl HandleRequest for Initialize {
    fn handle_request(
        mut st: ServerState,
        params: <Self as Request>::Params,
    ) -> impl Future<Output = Result<<Self as Request>::Result, ResponseError>> + Send + 'static
    {
        let ecf_path = params
            .initialization_options
            .map(|x| {
                x.as_object()
                    .map(|z| {
                        z.get("ecf_path").map(|path| {
                            PathBuf::try_from(path.as_str().expect("must be string"))
                                .expect("must be a valid path")
                        })
                    })
                    .flatten()
            })
            .flatten()
            .unwrap_or_else(|| {
                let cwd = env::current_dir().expect("fails to retrieve current working directory");
                let first_config_file = fs::read_dir(cwd)
                    .expect("fails to interate over current directory contents")
                    .filter(|file| {
                        file.as_ref().is_ok_and(|file| {
                            file.path().extension().is_some_and(|ext| ext == "ecf")
                        })
                    }).next().expect("fails to find any ecf in current working directory and the configuration has not been passed as initialization option").expect("fails to read at least one ecf file in current working directory");
                first_config_file.path()
            });
        let Some(system) = System::parse_from_file(&ecf_path) else {
            panic!("fails to read config file")
        };
        async move {
            st.add_task(system.into()).await;
            Ok(InitializeResult {
                capabilities: ServerCapabilities {
                    hover_provider: Some(HoverProviderCapability::Simple(true)),
                    definition_provider: Some(OneOf::Left(true)),
                    document_symbol_provider: Some(OneOf::Left(true)),
                    workspace_symbol_provider: Some(OneOf::Left(true)),
                    code_action_provider: Some(true.into()),
                    text_document_sync: Some(TextDocumentSyncCapability::Options(
                        TextDocumentSyncOptions {
                            save: Some(TextDocumentSyncSaveOptions::Supported(true)),
                            ..TextDocumentSyncOptions::default()
                        },
                    )),
                    ..ServerCapabilities::default()
                },
                server_info: None,
            })
        }
    }
}
