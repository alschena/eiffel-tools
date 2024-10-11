use super::common::{HandleNotification, HandleRequest, ServerState};
use crate::lib::config;
use async_lsp::lsp_types::{
    notification, request, HoverProviderCapability, InitializeResult, OneOf, ServerCapabilities,
};
use async_lsp::{ResponseError, Result};
use std::env;
use std::fs;
use std::future::Future;
use std::ops::ControlFlow;
impl HandleRequest for request::Initialize {
    fn handle_request(
        _st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        async move {
            eprintln!("Initialize with {params:?}");
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
impl HandleNotification for notification::Initialized {
    fn handle_notification(
        st: ServerState,
        params: <Self as notification::Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        {
            let mut write_workspace = st.workspace.write().unwrap();
            let cwd = env::current_dir().expect("Fails to retrieve current working directory");
            let config_files: Result<Vec<fs::DirEntry>, std::io::Error> = fs::read_dir(cwd)
                .expect("Fails to interate over current directory contents")
                .filter(|file| {
                    file.as_ref()
                        .is_ok_and(|file| file.path().extension().is_some_and(|ext| ext == "ecf"))
                })
                .collect();
            debug_assert!(config_files.is_ok());
            let config_files = config_files.unwrap();
            debug_assert_eq!(config_files.len(), 1);
            let config_file = config_files
                .first()
                .expect("Fails to find configuration in current working directory");
            let src_config =
                fs::read_to_string(config_file.path()).expect("Fails to read configuration file");
            let system: config::System =
                serde_xml_rs::from_str(&src_config).expect("Fails to parse configuration source");
            let eiffel_files = system
                .eiffel_files()
                .expect("Fails to extract eiffel files from system");
            for file in eiffel_files {
                write_workspace
                    .add_file(&file)
                    .expect(format!("Fails to add file: {:?} to workspace", &file).as_str())
            }
        }
        ControlFlow::Continue(())
    }
}
