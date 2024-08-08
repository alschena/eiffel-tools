use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use tree_sitter::{InputEdit, Language, Parser, Point, Tree, TreeCursor};

struct Workspace<'a> {
    classes: Vec<Class<'a>>,
    files: HashMap<PathBuf, Tree>,
    parser: Parser,
}

struct FileProcessor {
    tree: Tree,
    path: PathBuf,
    src: Vec<u8>,
}
impl FileProcessor {
    fn new(parser: &mut Parser, path: PathBuf) -> FileProcessor {
        let src = std::fs::read(&path).expect("Failed to read file.");
        let tree = parser.parse(&src, None).unwrap();
        FileProcessor { tree, path, src }
    }
    fn class(&mut self) -> Option<Class> {
        let mut cursor = self.tree.walk();
        self.class_helper(&mut cursor)
    }
    fn class_helper<'a>(&self, c: &mut TreeCursor) -> Option<Class<'a>> {
        if c.node().kind() == "class_name" {
            let name = match String::from_utf8(self.src[c.node().byte_range()].to_vec()) {
                Ok(v) => v,
                Err(e) => panic!("invalid UTF-8 sequence {}", e),
            };
            let source_file = self.path.clone();
            let features = Vec::new();
            let descendants = Vec::new();
            let ancestors = Vec::new();
            Some(Class {
                name,
                source_file,
                features,
                descendants,
                ancestors,
            })
        } else {
            if c.goto_next_sibling() {
                self.class_helper(c)
            } else {
                if c.goto_first_child() {
                    self.class_helper(c)
                } else {
                    None
                }
            }
        }
    }
    fn features(&mut self) -> Vec<Feature> {
        todo!()
    }
}

impl Workspace<'_> {
    fn new() -> Workspace<'static> {
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");

        Workspace {
            classes: Vec::new(),
            files: HashMap::new(),
            parser,
        }
    }
    fn add_file(&mut self, filepath: PathBuf) {}
    fn init_classes(&mut self) {}
}

/// Registers the class of a file. There must be a signle class per file.
fn init_class<'a>(cursor: &mut TreeCursor) -> Option<Class<'a>> {
    todo!()
}

fn init_features<'a>(cursor: &mut TreeCursor, acc: &mut Vec<Feature<'a>>) -> Vec<Feature<'a>> {
    debug_assert!(cursor.node().kind() == "feature_declaration");
    let declaration_node = cursor.node();
    if cursor.goto_first_child() {
        debug_assert!(cursor.node().kind() == "new_feature");
        acc.push(Feature {
            name: todo!(),
            visibility: FeatureVisibility::None,
        });
        while !cursor.goto_next_sibling() {
            if cursor.node().kind() == "new_feature" {
                todo!()
            }
        }
        cursor.reset(declaration_node);
        if cursor.goto_next_sibling() {
            init_features(cursor, acc);
        }
    }
    todo!();
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
    source_file: PathBuf,
    features: Vec<Feature<'a>>,
    descendants: Vec<&'a Class<'a>>,
    ancestors: Vec<&'a Class<'a>>,
}
