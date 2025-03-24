use crate::lib::code_entities::prelude::Point;
use crate::lib::generators::Generators;
use crate::lib::workspace::Workspace;
use async_lsp::lsp_types::request;
use async_lsp::ResponseError;
use std::future::Future;
use std::path::Path;

mod add_class_specification;
pub use add_class_specification::ClassSpecificationGenerator;

mod add_routine_specification;
pub use add_routine_specification::RoutineSpecificationGenerator;
use async_lsp::lsp_types;
use serde_json;

trait Command<'ws>: TryFrom<(&'ws Workspace, Vec<serde_json::Value>)> {
    const NAME: &'static str;
    const TITLE: &'static str;

    fn arguments(&self) -> Vec<serde_json::Value>;

    fn generate_edits(
        &self,
        generators: &Generators,
    ) -> impl Future<Output = anyhow::Result<lsp_types::WorkspaceEdit>>;

    fn is_called(name: &str) -> bool {
        name == Self::NAME
    }
    fn command(&self) -> lsp_types::Command {
        let title = Self::TITLE.to_string();
        let command = Self::NAME.to_string();
        let arguments = if self.arguments().is_empty() {
            None
        } else {
            Some(self.arguments())
        };
        lsp_types::Command {
            title,
            command,
            arguments,
        }
    }
}

pub enum Commands<'ws> {
    AddClassSpecification(ClassSpecificationGenerator<'ws>),
    AddRoutineSpecification(RoutineSpecificationGenerator<'ws>),
}

impl<'ws> Commands<'ws> {
    pub fn try_new(
        ws: &'ws Workspace,
        params: lsp_types::ExecuteCommandParams,
    ) -> anyhow::Result<Self> {
        let name = params.command;
        let args = params.arguments;

        if ClassSpecificationGenerator::is_called(&name) {
            let command = ClassSpecificationGenerator::try_from((ws, args))?;
            return Ok(Commands::AddClassSpecification(command));
        }
        if RoutineSpecificationGenerator::is_called(&name) {
            let command = RoutineSpecificationGenerator::try_from((ws, args))?;
            return Ok(Commands::AddRoutineSpecification(command));
        }
        unimplemented!()
    }
    pub fn try_new_add_routine_specification_at_cursor(
        ws: &'ws Workspace,
        filepath: &Path,
        cursor: Point,
    ) -> anyhow::Result<Self> {
        let command = RoutineSpecificationGenerator::try_new_at_cursor(ws, filepath, cursor)?;
        Ok(Commands::AddRoutineSpecification(command))
    }
    fn title(&self) -> String {
        match self {
            Commands::AddClassSpecification(_) => {
                return ClassSpecificationGenerator::TITLE.to_string()
            }
            Commands::AddRoutineSpecification(_) => {
                return RoutineSpecificationGenerator::TITLE.to_string()
            }
        }
    }
    async fn generate_edits(
        &self,
        generators: &Generators,
    ) -> anyhow::Result<lsp_types::WorkspaceEdit> {
        match self {
            Commands::AddClassSpecification(class_specification_generator) => {
                class_specification_generator
                    .generate_edits(generators)
                    .await
            }
            Commands::AddRoutineSpecification(routine_specification_generator) => {
                routine_specification_generator
                    .generate_edits(generators)
                    .await
            }
        }
    }
    async fn request_edits(
        &self,
        client: &async_lsp::ClientSocket,
        edit: lsp_types::WorkspaceEdit,
    ) -> Result<(), async_lsp::ResponseError> {
        let response = client
            .request::<request::ApplyWorkspaceEdit>(lsp_types::ApplyWorkspaceEditParams {
                label: Some(format!("Edits requested by {}", self.title())),
                edit,
            })
            .await
            .map_err(|e| {
                async_lsp::ResponseError::new(
                    async_lsp::ErrorCode::REQUEST_FAILED,
                    format!("fails with error: {e}"),
                )
            })?;
        if response.applied {
            Ok(())
        } else {
            let error = ResponseError::new(
                async_lsp::ErrorCode::REQUEST_FAILED,
                response.failure_reason.unwrap_or_else(|| {
                    "The client does not apply the workspace edits.".to_string()
                }),
            );
            Err(error)
        }
    }
    pub async fn run<'st>(
        &self,
        client: &'st async_lsp::ClientSocket,
        generators: &'st Generators,
    ) -> Result<(), async_lsp::ResponseError> {
        let edit = self.generate_edits(generators).await.map_err(|e| {
            async_lsp::ResponseError::new(
                async_lsp::ErrorCode::REQUEST_FAILED,
                format!("Fails to generate text edits with error: {e}"),
            )
        })?;
        self.request_edits(client, edit).await
    }
    pub async fn command(&self) -> lsp_types::Command {
        match self {
            Commands::AddClassSpecification(val) => val.command(),
            Commands::AddRoutineSpecification(val) => val.command(),
        }
    }
    pub fn list_names() -> Vec<String> {
        vec![
            RoutineSpecificationGenerator::NAME.to_string(),
            ClassSpecificationGenerator::NAME.to_string(),
        ]
    }
}
