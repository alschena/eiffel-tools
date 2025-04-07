use super::code_entities::prelude::*;
use super::parser::Parse;
use anyhow::{Context, Result};
use async_lsp::lsp_types;
use std::path::{Path, PathBuf};
use tracing::info;
use tracing::instrument;
use tracing::warn;
use tree_sitter::QueryCursor;
use tree_sitter::{Parser, Tree};

/// Stores all the information of a file
#[derive(Debug, Clone)]
pub struct ProcessedFile {
    /// Treesitter abstract syntax tree, stored for incremental parsing.
    pub tree: Tree,
    /// Path of the file
    pub path: PathBuf,
    /// In eiffel a class contains all other code entities of a class
    pub class: Class,
}
impl ProcessedFile {
    #[instrument(skip(parser))]
    pub(crate) async fn new(parser: &mut Parser, path: PathBuf) -> Option<ProcessedFile> {
        let src: String =
            String::from_utf8(tokio::fs::read(&path).await.expect("Failed to read file."))
                .expect("Source code must be UTF8 encoded");
        let tree = parser.parse(&src, None).unwrap();
        let Ok(class) =
            Class::parse_through(&tree.root_node(), &mut QueryCursor::new(), src.as_str())
                .context("parsing class")
        else {
            info!("fails to parse {:?}", &path);
            return None;
        };
        Some(ProcessedFile { tree, path, class })
    }
    pub(crate) fn tree(&self) -> &Tree {
        &self.tree
    }
    pub(crate) fn feature_around_point(&self, point: Point) -> Option<&Feature> {
        Feature::feature_around_point(self.class().features().iter(), point)
    }
    pub fn reload(&mut self, parser: &mut Parser) {
        let src = std::fs::read_to_string(self.path()).expect("read file.");
        let tree = parser.parse(&src, None).unwrap();
        Class::parse_through(&tree.root_node(), &mut QueryCursor::new(), src.as_str())
            .inspect(|_| {
                info!("reloading file at {:#?}", self.path());
            })
            .inspect_err(|e| {
                warn!(
                    "fails to reload file at path: {:#?}\n\twith error {e:#?}",
                    self.path()
                )
            })
            .ok()
            .map(|class| self.class = class);
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn class(&self) -> &Class {
        &self.class
    }
}

/// Compatibility with LSP types.
impl TryFrom<&ProcessedFile> for lsp_types::SymbolInformation {
    type Error = anyhow::Error;

    fn try_from(value: &ProcessedFile) -> std::result::Result<Self, Self::Error> {
        let class = value.class();
        let ClassName(name) = class.name().to_owned();
        let kind = lsp_types::SymbolKind::CLASS;
        let tags = None;
        let deprecated = None;
        let container_name = None;
        let location: lsp_types::Location =
            Location::new(value.path().to_path_buf()).to_lsp_location(class.range().clone())?;
        Ok(lsp_types::SymbolInformation {
            name,
            kind,
            tags,
            deprecated,
            location,
            container_name,
        })
    }
}
impl TryFrom<&ProcessedFile> for lsp_types::WorkspaceSymbol {
    type Error = anyhow::Error;

    fn try_from(value: &ProcessedFile) -> std::result::Result<Self, Self::Error> {
        let ClassName(name) = value.class().name().to_owned();
        let location = (&Location::new(value.path().to_path_buf())).try_into()?;
        Ok(lsp_types::WorkspaceSymbol {
            name,
            kind: lsp_types::SymbolKind::CLASS,
            container_name: None,
            location: lsp_types::OneOf::Right(location),
            data: None,
            tags: None,
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use assert_fs::prelude::*;
    use assert_fs::{fixture::FileWriteStr, TempDir};

    fn parser() -> tree_sitter::Parser {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        parser
    }

    #[tokio::test]
    async fn reload() {
        let mut parser = parser();
        let temp_dir = TempDir::new().expect("must create temporary directory.");
        let file = temp_dir.child("processed_file_new.e");
        file.write_str(
            r#"
class A
feature
  x: INTEGER
end
            "#,
        )
        .expect("write to file");
        assert!(file.exists());

        let mut processed_file = ProcessedFile::new(&mut parser, file.to_path_buf())
            .await
            .expect("processed file must be produced.");

        assert_eq!(file.to_path_buf(), processed_file.path());

        assert_eq!(processed_file.class().features().len(), 1);

        file.write_str(
            r#"
class A
feature
  x: INTEGER
  y: INTEGER
end
            "#,
        )
        .expect("temp file must be writable");

        processed_file.reload(&mut parser);

        assert_eq!(
            processed_file.class().features().len(),
            2,
            "after reload there are two parsed features."
        )
    }
}
