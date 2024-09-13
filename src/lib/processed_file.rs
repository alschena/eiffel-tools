use super::code_entities::Class;
use std::path::PathBuf;
use tree_sitter::{Parser, Tree};

pub(crate) struct ProcessedFile {
    pub(super) tree: Tree,
    pub(super) path: PathBuf,
    pub(super) src: String,
}
impl ProcessedFile {
    pub(crate) fn new(parser: &mut Parser, path: PathBuf) -> ProcessedFile {
        let src: String = String::from_utf8(std::fs::read(&path).expect("Failed to read file."))
            .expect("Source code must be UTF8 encoded");
        let tree = parser.parse(&src, None).unwrap();
        ProcessedFile { tree, path, src }
    }
}

impl From<&ProcessedFile> for Class<'_> {
    fn from(value: &ProcessedFile) -> Self {
        let mut class = Class::from_tree_and_src(&value.tree, &value.src);
        class.add_location(&value.path);
        class
    }
}

#[cfg(test)]
mod tests {
    use crate::lib::processed_file::ProcessedFile;
    use crate::lib::tree_sitter::WidthFirstTraversal;
    use std::fs::File;
    use std::io::prelude::*;
    use std::path::PathBuf;

    const PROCEDURE_PATH: &str = "/tmp/class_with_feature_path.e";
    const PROCEDURE: &str = "
class A feature
  f(x, y: INTEGER; z: BOOLEAN)
    do
    end
end
";

    #[test]
    fn process_procedure() -> std::io::Result<()> {
        let procedure_path: PathBuf = PathBuf::from(PROCEDURE_PATH);
        let mut file = File::create(&procedure_path)?;
        file.write_all(PROCEDURE.as_bytes())?;

        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        let file = ProcessedFile::new(&mut parser, procedure_path.clone());

        let cursor = file.tree.walk();
        let mut width_first = WidthFirstTraversal::new(cursor);

        assert_eq!(
            width_first.next().expect("source file node").kind(),
            "source_file"
        );
        assert_eq!(
            width_first.next().expect("class declaration node").kind(),
            "class_declaration"
        );
        assert_eq!(width_first.next().expect("class").kind(), "class");
        assert_eq!(width_first.next().expect("class_name").kind(), "class_name");
        assert_eq!(
            width_first.next().expect("feature clause").kind(),
            "feature_clause"
        );

        Ok(())
    }
}
