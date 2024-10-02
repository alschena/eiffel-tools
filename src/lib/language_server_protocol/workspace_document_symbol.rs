use super::common::{HandleRequest, ServerState};
use crate::lib::code_entities::class::Class;
use crate::lib::code_entities::shared::*;
use async_lsp::lsp_types::{
    request, DocumentSymbol, OneOf, SymbolInformation, SymbolKind, WorkspaceLocation,
    WorkspaceSymbol, WorkspaceSymbolResponse,
};
use async_lsp::ResponseError;
use async_lsp::Result;
use std::future::Future;
impl TryFrom<&Class> for WorkspaceSymbol {
    type Error = anyhow::Error;

    fn try_from(value: &Class) -> std::result::Result<Self, Self::Error> {
        let name = value.name().to_string();
        let features = value.features();
        let children: Option<Vec<DocumentSymbol>> = Some(
            features
                .into_iter()
                .map(|x| DocumentSymbol::try_from(x.as_ref()))
                .collect::<anyhow::Result<Vec<DocumentSymbol>>>()?,
        );
        let location = match value.location() {
            Some(v) => v.try_into()?,
            None => anyhow::bail!("Expected class with valid file location"),
        };
        Ok(WorkspaceSymbol {
            name,
            kind: SymbolKind::CLASS,
            tags: None,
            container_name: None,
            location: OneOf::Right(location),
            data: None,
        })
    }
}
impl TryFrom<&Location> for WorkspaceLocation {
    type Error = anyhow::Error;
    fn try_from(value: &Location) -> Result<Self, Self::Error> {
        match value.try_into() {
            Err(e) => Err(e),
            Ok(uri) => Ok(Self { uri }),
        }
    }
}
impl HandleRequest for request::WorkspaceSymbolRequest {
    fn handle_request(
        st: ServerState,
        params: <Self as request::Request>::Params,
    ) -> impl Future<Output = Result<<Self as request::Request>::Result, ResponseError>> + Send + 'static
    {
        async move {
            let read_workspace = st.workspace.read().unwrap();
            let classes: Vec<Class> = read_workspace
                .iter()
                .map(|x| x.try_into().expect("Parse class"))
                .collect();
            let symbol_information: Vec<SymbolInformation> = classes
                .iter()
                .map(|x| {
                    <SymbolInformation>::try_from(x)
                        .expect("Class convertable to symbol information")
                })
                .collect();
            Ok(Some(WorkspaceSymbolResponse::Flat(symbol_information)))
        }
    }
}

#[cfg(test)]
mod test {

    use super::*;
    use crate::lib::processed_file;
    use std::fs::File;
    use std::io::prelude::*;
    use std::path::PathBuf;

    #[test]
    fn class_to_workspacesymbol() {
        let path = "/tmp/eiffel_tool_test_class_to_workspacesymbol.e";
        let path = PathBuf::from(path);
        let src = "
    class A
    note
    end
        ";
        let mut file = File::create(path.clone()).expect("Failed to create file");
        file.write_all(src.as_bytes())
            .expect("Failed to write to file");
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");
        let file = processed_file::ProcessedFile::new(&mut parser, path.clone());
        let class: Class = (&file).try_into().expect("Parse class");
        let symbol = <WorkspaceSymbol>::try_from(&class);
        assert!(symbol.is_ok())
    }
}
