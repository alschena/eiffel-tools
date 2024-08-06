use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use tree_sitter::{InputEdit, Language, Parser, Point, Tree};

struct Workspace<'a> {
    classes: Vec<Class<'a>>,
    parsed_files: HashMap<PathBuf, Tree>,
    parser: Parser,
}

impl Workspace<'_> {
    fn new() -> Workspace<'static> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        Workspace {
            classes: Vec::new(),
            parsed_files: HashMap::new(),
            parser,
        }
    }
    fn add_file(&mut self, filepath: PathBuf) {
        let mut f = File::open(&filepath).expect("Failed to open file.");
        let mut src = String::new();
        f.read_to_string(&mut src).expect("Failed to read file.");
        let tree = self.parser.parse(src, None).unwrap();
        self.parsed_files.insert(filepath, tree);
    }
}

#[derive(Debug)]
enum FeatureVisibility<'a> {
    None,
    Some(&'a Class<'a>),
    All,
}

#[derive(Debug)]
struct Feature<'a> {
    name: String,
    visibility: FeatureVisibility<'a>,
}

#[derive(Debug)]
struct Class<'a> {
    name: String,
    source_file: &'a Path,
    features: Vec<Feature<'a>>,
    descendants: Vec<&'a Class<'a>>,
    ancestors: Vec<&'a Class<'a>>,
}
