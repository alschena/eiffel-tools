use super::code_entities::prelude::*;
use super::tree_sitter_extension::Parse;
use anyhow::{Context, Result};
use async_lsp::lsp_types;
use std::path::{Path, PathBuf};
use tracing::info;
use tracing::instrument;
use tree_sitter::QueryCursor;
use tree_sitter::{Parser, Tree};

/// Stores all the information of a file
#[derive(Debug, Clone)]
pub struct ProcessedFile {
    /// Treesitter abstract syntax tree, stored for incremental parsing.
    tree: Tree,
    /// Path of the file
    path: PathBuf,
    /// In eiffel a class contains all other code entities of a class
    class: Class,
}
impl ProcessedFile {
    #[instrument(skip(parser))]
    pub(crate) async fn new(parser: &mut Parser, path: PathBuf) -> Option<ProcessedFile> {
        let src: String =
            String::from_utf8(tokio::fs::read(&path).await.expect("Failed to read file."))
                .expect("Source code must be UTF8 encoded");
        let tree = parser.parse(&src, None).unwrap();
        let Ok(class) = Class::parse(&tree.root_node(), &mut QueryCursor::new(), src.as_str())
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
    pub(crate) fn feature_around_point(&self, point: &Point) -> Option<&Feature> {
        let mut features = self.class().features().iter();
        match features
            .find(|&feature| point >= feature.range().start() && point <= feature.range().end())
        {
            Some(f) => Some(f),
            None => None,
        }
    }
    pub fn path(&self) -> &Path {
        &self.path
    }
    pub(crate) fn class(&self) -> &Class {
        &self.class
    }
    pub fn feature_src_with_injections<'a>(
        &self,
        feature: &Feature,
        injections: impl Iterator<Item = (&'a Point, &'a str)> + Clone,
    ) -> Result<String> {
        debug_assert!(injections.clone().is_sorted_by(|(a, _), (b, _)| { a <= b }));

        let src = String::from_utf8(std::fs::read(self.path())?)?;
        let range = feature.range();
        let start = range.start();
        let end = range.end();

        let mut injections = injections.peekable();

        let mut feature_src = String::new();
        src.lines()
            .enumerate()
            .skip(start.row)
            .take((end.row - start.row) + 1)
            .for_each(|(linenum, line)| match injections.peek() {
                Some((&Point { row, column: _ }, text)) if row < linenum => {
                    feature_src.push_str(text);
                    injections.next();
                }
                Some((&Point { row, column }, text)) if row == linenum => {
                    feature_src.push_str(&line[..column]);
                    feature_src.push_str(text);
                    feature_src.push_str(&line[column..]);
                    feature_src.push('\n');
                    injections.next();
                }
                _ => {
                    feature_src.push_str(line);
                    feature_src.push('\n');
                }
            });
        Ok(feature_src)
    }
}

/// Compatibility with LSP types.
impl TryFrom<&ProcessedFile> for lsp_types::SymbolInformation {
    type Error = anyhow::Error;

    fn try_from(value: &ProcessedFile) -> std::result::Result<Self, Self::Error> {
        let class = value.class();
        let name = class.name().into();
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
        let name = value.class().name().to_string();
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
    #[tokio::test]
    async fn new() {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");

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
        .expect("temp file must be writable");
        assert!(file.exists());
        let processed_file = ProcessedFile::new(&mut parser, file.to_path_buf())
            .await
            .expect("processed file must be produced.");
        assert_eq!(file.to_path_buf(), processed_file.path());
        assert_eq!("A", processed_file.class().name());
    }
    #[tokio::test]
    async fn feature_str_with_injections() {
        let temp_dir = TempDir::new().expect("must create temporary directory.");
        let file = temp_dir.child("processed_file_new.e");

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        file.write_str(
            r#"
class A feature
  f(x, y: INTEGER; z: BOOLEAN)
    do
    end
end
            "#,
        )
        .expect("temp file must be writable");
        assert!(file.exists());
        let processed_file = ProcessedFile::new(&mut parser, file.to_path_buf())
            .await
            .expect("An instance of processed file is produced.");
        let feature = processed_file
            .class
            .features()
            .first()
            .expect("There is a feature");
        let range = feature.range();
        let begin = feature.range().start();
        let end = feature.range().end();

        let text_with_injection = processed_file
            .feature_src_with_injections(
                feature,
                vec![(begin, "[FIRST_LINE_OF_FEATURE] ")].into_iter(),
            )
            .expect("the injections must succeed");
        assert_eq!(
            r#"  [FIRST_LINE_OF_FEATURE] f(x, y: INTEGER; z: BOOLEAN)
    do
    end
"#,
            text_with_injection
        );
    }
}
