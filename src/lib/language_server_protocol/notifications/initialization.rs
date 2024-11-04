use crate::lib::config::{self, System};
use crate::lib::language_server_protocol::prelude::*;
use crate::lib::processed_file::ProcessedFile;
use async_lsp::lsp_types::notification::{Initialized, Notification};
use async_lsp::Result;
use rayon::prelude::*;
use std::env;
use std::fs;
use std::ops::ControlFlow;
use tracing::info;
impl HandleNotification for Initialized {
    fn handle_notification(
        st: ServerState,
        params: <Self as Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
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
        let Some(system) = System::parse_from_file(&config_file.path()) else {
            panic!("fails to read config file")
        };
        let eiffel_files = system
            .eiffel_files()
            .expect("Fails to extract eiffel files from system");
        let files = eiffel_files
            .par_iter()
            .filter_map(|filepath| {
                let mut parser = tree_sitter::Parser::new();
                parser
                    .set_language(&tree_sitter_eiffel::LANGUAGE.into())
                    .expect("Error loading Eiffel grammar");
                match ProcessedFile::new(&mut parser, filepath.to_owned()) {
                    Ok(f) => Some(f),
                    Err(_) => {
                        info!("fails to parse: {:?}", filepath);
                        None
                    }
                }
            })
            .collect();
        let mut write_workspace = st.workspace.write().expect("workspace must be writable");
        write_workspace.set_files(files);
        ControlFlow::Continue(())
    }
}
