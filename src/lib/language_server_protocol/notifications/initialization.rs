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
        mut st: ServerState,
        params: <Self as Notification>::Params,
    ) -> ControlFlow<Result<()>, ()> {
        ControlFlow::Continue(())
    }
}
