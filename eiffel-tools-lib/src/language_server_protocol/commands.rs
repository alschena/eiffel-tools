use crate::code_entities::prelude::Point;
use crate::generators::Generators;
use crate::workspace::Workspace;
use anyhow::Context;
use anyhow::Result;
use async_lsp::ResponseError;
use async_lsp::lsp_types;
use async_lsp::lsp_types::request;
use serde_json;
use std::future::Future;
use std::path::Path;

mod add_class_specification;
use add_class_specification::ClassSpecificationGenerator;

mod add_routine_specification;
use add_routine_specification::RoutineSpecificationGenerator;

mod add_daikon_instrumentation;
use add_daikon_instrumentation::DaikonInstrumenter;

mod fix_routine;
pub use fix_routine::FixRoutine;

pub trait Command<'ws>: TryFrom<(&'ws Workspace, Vec<serde_json::Value>)> {
    const NAME: &'static str;
    const TITLE: &'static str;

    fn arguments(&self) -> Vec<serde_json::Value>;

    fn generate_edits(
        &self,
        _generators: &Generators,
    ) -> impl Future<Output = Result<Option<lsp_types::WorkspaceEdit>>> {
        async { Ok(None) }
    }

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

    fn side_effect(
        &mut self,
        _generators: &Generators,
    ) -> impl std::future::Future<Output = anyhow::Result<()>> + Send {
        async { Ok(()) }
    }

    fn request_edits(
        &self,
        client: &async_lsp::ClientSocket,
        edit: lsp_types::WorkspaceEdit,
    ) -> impl std::future::Future<Output = Result<(), async_lsp::ResponseError>> + Send
    where
        Self: Sync,
    {
        async {
            let response = client
                .request::<request::ApplyWorkspaceEdit>(lsp_types::ApplyWorkspaceEditParams {
                    label: Some(format!("Edits requested by {:#?}", self.command())),
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
    }
}

macro_rules! commands {
    (
    name: $enum_name:ident;
    variants: $variants:tt
    $(; functions: $($fn_name:ident $fn_params:tt -> $fn_ret_type:ty),+)?
    $(; async_functions: $($async_fn_name:ident $async_fn_params:tt -> $async_fn_ret_type:ty),+)?
    $(; async_functions_mut: $($async_fn_name_mut:ident $async_fn_params_mut:tt -> $async_fn_ret_type_mut:ty),+)?
     ) => {
        commands!(@def_enum $enum_name, $variants);

        impl<'ws> $enum_name<'ws> {
            pub fn try_new(
                ws: &'ws Workspace,
                params: lsp_types::ExecuteCommandParams,
            ) -> anyhow::Result<Self> {
                let name = params.command;
                let args = params.arguments;

                commands!(@create $enum_name, $variants, name, args, ws);
                unimplemented!()
            }

            pub fn list_names() -> Vec<String> {
                commands!(@list_variants_static $variants, NAME)
            }

            $(commands!(@functions $enum_name, self.[$($fn_name $fn_params -> $fn_ret_type),+], $variants);)?

            $(commands!(@async_functions $enum_name, self.[$($async_fn_name $async_fn_params -> $async_fn_ret_type),+], $variants);)?

            $(commands!(@async_functions_mut $enum_name, self.[$($async_fn_name_mut $async_fn_params_mut -> $async_fn_ret_type_mut),+], $variants);)?
        }
    };

    (@def_enum $enum_name:ident, [$($variants:ident),+] ) => {
        #[derive(Debug, Clone)]
        pub enum $enum_name<'ws> {
            $($variants($variants<'ws>)),*
        }
    };

    (@create $enum_name:ident, [$($variant:ident),+], $name:ident, $arguments:ident, $workspace:ident) => {
        $(if $variant::is_called(&$name) {
            let command = $variant::try_from(($workspace,$arguments))?;
            return Ok($enum_name::$variant(command));
        })+
    };

    (@list_variants_static [$($variant:ident),+], $method:ident) => {
        vec![$($variant::$method.to_string()),+]
    };

    (@functions $enum_name:ident, $self:ident.[$($func:ident ($($param_name:ident : $param_type:ty),*) -> $ret_type:ty),+], $variants:tt) => {
        $(pub fn $func(&$self $(, $param_name : $param_type)*) -> $ret_type {
            commands!(@match_variants $enum_name, $self, $variants, ($func ($($param_name),*) -> $ret_type))
        })+
    };

    (@async_functions $enum_name:ident, $self:ident.[$($func:ident ($($param_name:ident : $param_type:ty),*) -> $ret_type:ty),+], $variants:tt) => {
        $(pub async fn $func(&$self $(, $param_name : $param_type)*) -> $ret_type {
            commands!(@async_match_variants $enum_name, $self, $variants, ($func ($($param_name),*) -> $ret_type))
        })+
    };

    (@async_functions_mut $enum_name:ident, $self:ident.[$($func:ident ($($param_name:ident : $param_type:ty),*) -> $ret_type:ty),+], $variants:tt) => {
        $(pub async fn $func(&mut $self $(, $param_name : $param_type)*) -> $ret_type {
            commands!(@async_match_variants_mut $enum_name, $self, $variants, ($func ($($param_name),*) -> $ret_type))
        })+
    };

    (@match_variants $enum_name:ident, $self:ident, [$($variant:ident),+], ($func:ident $params_names:tt -> $ret_type:ty)) => {
        match $self {
            $($enum_name::$variant(inner) => {commands!(@function_call $variant $func $params_names inner)}),+
        }

    };

    (@async_match_variants $enum_name:ident, $self:ident, [$($variant:ident),+], ($func:ident $params_names:tt -> $ret_type:ty)) => {
        match $self {
            $($enum_name::$variant(inner) => {commands!(@async_function_call $variant $func $params_names inner)}),+
        }

    };

    (@async_match_variants_mut $enum_name:ident, $self:ident, [$($variant:ident),+], ($func:ident $params_names:tt -> $ret_type:ty)) => {
        match $self {
            $($enum_name::$variant(inner) => {commands!(@async_function_call $variant $func $params_names inner)}),+
        }

    };

    (@function_call $variant:ident $func:ident ($($param_name:ident),*) $target:ident) => {
        $variant::$func($target $(, $param_name)*)
    };

    (@async_function_call $variant:ident $func:ident ($($param_name:ident),*) $target:ident) => {
        $variant::$func($target $(, $param_name)*).await
    };
}

commands!(
    name: Commands;
    variants: [ClassSpecificationGenerator, RoutineSpecificationGenerator, DaikonInstrumenter, FixRoutine];
    functions:
        command() -> lsp_types::Command;
    async_functions:
        generate_edits(g: &Generators) -> Result<Option<lsp_types::WorkspaceEdit>>,
        request_edits(
            client: &async_lsp::ClientSocket,
            edit: lsp_types::WorkspaceEdit
        ) -> Result<(), async_lsp::ResponseError>;
    async_functions_mut:
        side_effect(g: &Generators) -> Result<()>
);

impl<'ws> Commands<'ws> {
    pub fn try_new_add_routine_specification_at_cursor(
        ws: &'ws Workspace,
        filepath: &'ws Path,
        cursor: Point,
    ) -> Result<Self> {
        let command = RoutineSpecificationGenerator::try_new_at_cursor(ws, filepath, cursor)?;
        Ok(Commands::RoutineSpecificationGenerator(command))
    }

    pub fn try_new_instrument_routine_at_cursor_for_daikon(
        ws: &'ws Workspace,
        filepath: &'ws Path,
        cursor: Point,
    ) -> anyhow::Result<Self> {
        let feature = ws.feature_around(filepath, cursor).with_context(|| {
            format!(
                "fails to find feature around point {:#?} at path: {:#?}.",
                cursor, filepath,
            )
        })?;

        let command = DaikonInstrumenter::try_new(ws, filepath, feature.name())?;
        Ok(Commands::DaikonInstrumenter(command))
    }

    pub fn try_new_fix_routine(
        ws: &'ws Workspace,
        filepath: &'ws Path,
        cursor: Point,
    ) -> anyhow::Result<Self> {
        let feature = ws.feature_around(filepath, cursor).with_context(|| {
            format!(
                "fails to find feature around point {:#?} at path: {:#?}.",
                cursor, filepath,
            )
        })?;

        let command = FixRoutine::try_new(ws, filepath, feature.name())?;
        Ok(Commands::FixRoutine(command))
    }

    pub async fn run<'st>(
        &mut self,
        client: &'st async_lsp::ClientSocket,
        generators: &'st Generators,
    ) -> Result<(), async_lsp::ResponseError> {
        self.side_effect(generators).await.map_err(|e| {
            async_lsp::ResponseError::new(
                async_lsp::ErrorCode::REQUEST_FAILED,
                format!("Fails to execute commands side effects with error: {e}"),
            )
        })?;

        if let Some(edit) = self.generate_edits(generators).await.map_err(|e| {
            async_lsp::ResponseError::new(
                async_lsp::ErrorCode::REQUEST_FAILED,
                format!("Fails to generate text edits with error: {e}"),
            )
        })? {
            self.request_edits(client, edit).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::generators::Generators;
    use anyhow::Context;
    use async_lsp::lsp_types::WorkspaceEdit;
    mod command_mock;
    use command_mock::MockCommand;

    commands!(
        name: CommandsTest;
        variants: [MockCommand];
        functions: command() -> lsp_types::Command, test_function_with_arg(s: String)-> String;
        async_functions: generate_edits(g: &Generators) -> Result<Option<WorkspaceEdit>>
    );

    #[test]
    fn command_enum_command() {
        let ws = Workspace::mock();
        let commands = CommandsTest::MockCommand(MockCommand::new(&ws));

        let command = commands.command();
        eprintln!("command: {command:#?}");
        eprintln!("command_enum: {commands:#?}");
        assert_eq!(command.title, "Mock");
        assert_eq!(command.command, "mock");
        assert_eq!(command.arguments, None);
    }

    #[tokio::test]
    async fn enum_command_generate_edits() {
        let ws = Workspace::mock();
        let commands = CommandsTest::MockCommand(MockCommand::new(&ws));

        let generators = Generators::mock();
        let generations = commands.generate_edits(&generators).await;

        assert!(generations.is_ok());

        let generations = generations.unwrap();
        assert!(generations.is_none());
    }

    #[test]
    fn constructor_command_enum() -> anyhow::Result<()> {
        let ws = Workspace::mock();
        let params = lsp_types::ExecuteCommandParams {
            command: "mock".to_string(),
            arguments: Vec::new(),
            ..Default::default()
        };
        let _ = CommandsTest::try_new(&ws, params).with_context(
            || "fails to create a commands from mock workspace and execute_command_parameters.",
        )?;
        Ok(())
    }

    #[test]
    /// The real test is the correct compilation of `test_function_with_arg`
    fn command_enum_function_with_params() {
        let ws = Workspace::mock();
        CommandsTest::MockCommand(MockCommand::new(&ws))
            .test_function_with_arg(String::from("Test"));
    }

    #[test]
    fn list_command_enum() {
        let list = CommandsTest::list_names();
        eprintln!("list of commands: {list:#?}");
        assert!(list.contains(&MockCommand::NAME.to_string()));
    }
}
