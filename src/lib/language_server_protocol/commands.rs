use crate::lib::code_entities::prelude::Point;
use crate::lib::generators::Generators;
use crate::lib::workspace::Workspace;
use async_lsp::lsp_types::request;
use async_lsp::ResponseError;
use std::future::Future;
use std::path::Path;

#[cfg(test)]
pub mod command_mock;
#[cfg(test)]
use command_mock::MockCommand;

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

macro_rules! impl_commands {
    (
    $enum_name:ident;
    $variants:tt;
    $(functions: $($fn_name:ident $fn_params:tt -> $fn_ret_type:ty),+)?
     ) => {
        impl_commands!(@def_enum $enum_name, $variants);
        impl<'ws> $enum_name<'ws> {
            pub fn try_new(
                ws: &'ws Workspace,
                params: lsp_types::ExecuteCommandParams,
            ) -> anyhow::Result<Self> {
                let name = params.command;
                let args = params.arguments;

                #[cfg(test)]
                impl_commands!(@create $enum_name, [MockCommand], name, args, ws);
                impl_commands!(@create $enum_name, $variants, name, args, ws);
                unimplemented!()
            }
            $(impl_commands!(@functions $enum_name, self.[$($fn_name $fn_params -> $fn_ret_type),+], $variants))?;
        }
    };
    (@create $enum_name:ident, [$($variant:ident),+], $name:ident, $arguments:ident, $workspace:ident) => {
        $(if $variant::is_called(&$name) {
            let command = $variant::try_from(($workspace,$arguments))?;
            return Ok($enum_name::$variant(command));
        })+
    };
    (@def_enum $enum_name:ident, [$($variants:ident),+] ) => {
        #[derive(Debug, Clone)]
        pub enum $enum_name<'ws> {
            $($variants($variants<'ws>)),*,
            #[cfg(test)]
            MockCommand(MockCommand),
        }
        #[cfg(test)]
        impl $enum_name<'_> {
            fn mock() -> Self {
                Self::MockCommand(MockCommand::default())
            }
        }
    };
    (@functions $enum_name:ident, $self:ident.[$($func:ident ($($param_name:ident : $param_type:ty),*) -> $ret_type:ty),+], $variants:tt) => {
        $(fn $func(&$self $(, $($param_name : $param_type),+)?) -> $ret_type {
            impl_commands!(@match_variants $enum_name, $self, $variants, $func ($($param_name),*) -> $ret_type)
        })+
    };
    (@match_variants $enum_name:ident, $self:ident, [$($variant:ident),+], $func:ident $params_names:tt -> $ret_type:ty) => {
        match $self {
            #[cfg(test)]
            $enum_name::MockCommand(ref inner) => MockCommand::$func(impl_commands!(@args inner $params_names)),
            $($enum_name::$variant(ref inner) => {$variant::$func(impl_commands!(@args inner $params_names))}),+
        }

    };
    (@args $target:ident ($($param_name:ident),*)) => {
        $target $(, $($param_name),+)?
    };
}

impl_commands!(
    CommandsEnum;
    [ClassSpecificationGenerator, RoutineSpecificationGenerator];
    functions: command() -> lsp_types::Command
);

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
        // apply_on_any!(generate_edits(generators).await);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::workspace::tests::*;

    #[test]
    fn command_enum_command() {
        let ws = Workspace::mock();
        let commands_enum = CommandsEnum::mock();

        let command = commands_enum.command();
        eprintln!("command: {command:#?}");
        eprintln!("command_enum: {commands_enum:#?}");
        assert_eq!(command.title, "Mock");
        assert_eq!(command.command, "mock");
        assert_eq!(command.arguments, None);
    }

    #[test]
    fn constructor_command_enum() {
        let ws = Workspace::mock();
        let params = lsp_types::ExecuteCommandParams {
            command: "mock".to_string(),
            arguments: Vec::new(),
            ..Default::default()
        };
        let command = CommandsEnum::try_new(&ws, params);
        eprintln!("{command:#?}");
    }
}
