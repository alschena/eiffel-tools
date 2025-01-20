use super::prelude::*;
use crate::lib::tree_sitter_extension::{capture_name_to_nodes, node_to_text, Parse};
use anyhow::anyhow;
use async_lsp::lsp_types;
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::Display;
use streaming_iterator::StreamingIterator;
use tracing::instrument;
use tree_sitter::{Node, QueryCursor};

// TODO accept only attributes of logical type in the model
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Model(Vec<Feature>);
impl Model {
    fn from_model_names(names: ModelNames, features: &Vec<Feature>) -> Model {
        Model(
            names
                .0
                .iter()
                .filter_map(|name| {
                    features
                        .iter()
                        .find(|feature| feature.name() == name)
                        .cloned()
                })
                .collect(),
        )
    }
}
impl Indent for Model {
    const INDENTATION_LEVEL: usize = 1;
}
impl Display for Model {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let display_text = self.0.iter().fold(String::new(), |mut acc, feature| {
            if !acc.is_empty() {
                acc.push(',');
                acc.push(' ');
            }
            acc.push_str(format!("{feature}").as_str());
            acc
        });
        write!(f, "{display_text}")
    }
}
#[derive(Debug, PartialEq, Eq, Clone)]
struct ModelNames(Vec<String>);
impl Parse for ModelNames {
    type Error = anyhow::Error;

    fn parse(node: &Node, query_cursor: &mut QueryCursor, src: &str) -> Result<Self, Self::Error> {
        let name_query = Self::query(
            r#"(class_declaration
            (notes (note_entry
                (tag) @tag
                value: (_) @id
                ("," value: (_) @id)*))
            (#eq? @tag "model"))"#,
        );

        let mut matches = query_cursor.matches(&name_query, *node, src.as_bytes());

        let mut names: Vec<String> = Vec::new();
        while let Some(mat) = matches.next() {
            capture_name_to_nodes("id", &name_query, mat)
                .for_each(|node| names.push(node_to_text(&node, src).to_string()));
        }

        Ok(ModelNames(names))
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Class {
    name: String,
    model: Model,
    features: Vec<Feature>,
    parents: Vec<Parent>,
    range: Range,
}

impl Class {
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn model(&self) -> &Model {
        &self.model
    }
    pub fn full_model<'a>(
        &'a self,
        system_classes: &'a [&'a Class],
    ) -> impl Iterator<Item = &'a Model> {
        self.parent_classes(system_classes)
            .map(|ancestor| ancestor.model())
            .chain(std::iter::once(self.model()))
    }
    pub fn features(&self) -> &Vec<Feature> {
        &self.features
    }
    pub fn into_features(self) -> Vec<Feature> {
        self.features
    }
    fn parents(&self) -> &Vec<Parent> {
        &self.parents
    }
    fn parent_classes<'a>(
        &'a self,
        system_classes: &'a [&'a Class],
    ) -> impl Iterator<Item = &'a Class> {
        self.parents()
            .into_iter()
            .filter_map(|parent| parent.class(system_classes))
    }
    pub fn ancestors<'a>(&'a self, system_classes: &'a [&'a Class]) -> HashSet<&'a Parent> {
        let mut ancestors = HashSet::new();
        for parent in self.parents() {
            let Some(parent_class) = parent.class(system_classes) else {
                continue;
            };
            ancestors.insert(parent);
            ancestors.extend(parent_class.ancestors(system_classes));
        }
        ancestors
    }
    pub fn ancestor_classes<'a>(&'a self, system_classes: &'a [&'a Class]) -> HashSet<&'a Class> {
        let mut ancestors_classes = HashSet::new();
        self.parent_classes(system_classes)
            .for_each(|parent_class| {
                ancestors_classes.insert(parent_class);
                ancestors_classes.extend(parent_class.ancestor_classes(system_classes));
            });
        ancestors_classes
    }
    pub fn inhereted_features<'a>(
        &'a self,
        system_classes: &'a [&'a Class],
    ) -> Vec<Cow<'a, Feature>> {
        self.parent_classes(system_classes)
            .into_iter()
            .zip(self.parents())
            .flat_map(|(parent_class, parent)| {
                parent_class
                    .inhereted_features(system_classes)
                    .into_iter()
                    .chain(parent_class.features().iter().map(|f| Cow::Borrowed(f)))
                    .map(|feature| {
                        match parent
                            .rename
                            .iter()
                            .find(|(old_name, _)| old_name == feature.name())
                        {
                            Some((_, new_name)) => {
                                Cow::Owned(feature.clone_rename(new_name.to_string()))
                            }
                            None => feature,
                        }
                    })
            })
            .collect()
    }
    pub fn range(&self) -> &Range {
        &self.range
    }

    pub fn from_name_range(name: String, range: Range) -> Class {
        Class {
            name,
            model: Model(Vec::new()),
            features: Vec::new(),
            parents: Vec::new(),
            range,
        }
    }

    pub fn add_feature(&mut self, feature: &Feature) {
        self.features.push(feature.clone())
    }

    pub fn add_model(&mut self, model: &Model) {
        self.model = model.clone()
    }

    #[cfg(test)]
    pub fn add_parent(&mut self, parent: Parent) {
        self.parents.push(parent)
    }
    #[cfg(test)]
    pub fn from_source(source: &str) -> Class {
        let mut parser = ::tree_sitter::Parser::new();
        let lang = tree_sitter_eiffel::LANGUAGE.into();
        parser
            .set_language(&lang)
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(source, None).unwrap();
        Class::parse(&tree.root_node(), &mut QueryCursor::new(), source)
            .expect("fails to parse class from given source code.")
    }
}
impl Indent for Class {
    const INDENTATION_LEVEL: usize = 1;
}

impl Parse for Class {
    type Error = anyhow::Error;

    #[instrument(skip_all)]
    fn parse(node: &Node, cursor: &mut QueryCursor, src: &str) -> anyhow::Result<Self> {
        let query = Self::query(
            "(class_declaration
            (class_name) @name
            (inheritance (parent)* @parent)*
            (feature_clause (feature_declaration)* @feature)*) @class",
        );

        let mut matches = cursor.matches(&query, *node, src.as_bytes());
        let class_match = matches.next().ok_or(anyhow!("File has no class."))?;

        let name = node_to_text(
            &capture_name_to_nodes("name", &query, class_match)
                .next()
                .expect("Each class has a name."),
            src,
        )
        .to_string();

        let range = capture_name_to_nodes("class", &query, class_match)
            .next()
            .expect("Class match has no class capture")
            .range()
            .into();

        let parents: Vec<Parent> = capture_name_to_nodes("parent", &query, class_match)
            .map(|ref node| {
                Parent::parse(node, &mut QueryCursor::new(), src).expect("error parsing parent.")
            })
            .collect();

        let features: Vec<Feature> = capture_name_to_nodes("feature", &query, class_match)
            .filter_map(|ref node| Feature::parse(node, &mut QueryCursor::new(), src).ok())
            .collect();

        let model = Model::from_model_names(ModelNames::parse(node, cursor, src)?, &features);
        Ok(Class {
            name,
            model,
            features,
            parents,
            range,
        })
    }
}
impl TryFrom<&Class> for lsp_types::DocumentSymbol {
    type Error = anyhow::Error;

    fn try_from(value: &Class) -> std::result::Result<Self, Self::Error> {
        let name = value.name().to_string();
        let features = value.features();
        let range = value.range().clone().try_into()?;
        let children: Option<Vec<lsp_types::DocumentSymbol>> = Some(
            features
                .into_iter()
                .map(|x| x.try_into().expect("feature conversion to document symbol"))
                .collect(),
        );
        Ok(lsp_types::DocumentSymbol {
            name,
            detail: None,
            kind: lsp_types::SymbolKind::CLASS,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children,
        })
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Parent {
    name: String,
    select: Vec<String>,
    rename: Vec<(String, String)>,
    redefine: Vec<String>,
    undefine: Vec<String>,
}
impl Parent {
    fn name(&self) -> &str {
        &self.name
    }
    pub fn class<'a>(&self, system_classes: &'a [&'a Class]) -> Option<&'a Class> {
        system_classes
            .into_iter()
            .find(|class| class.name() == self.name())
            .copied()
    }
    #[cfg(test)]
    pub fn from_name(name: String) -> Parent {
        Parent {
            name,
            select: Vec::new(),
            rename: Vec::new(),
            redefine: Vec::new(),
            undefine: Vec::new(),
        }
    }
}

impl Parse for Parent {
    type Error = anyhow::Error;

    #[instrument(skip_all)]
    fn parse(node: &Node, cursor: &mut QueryCursor, src: &str) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "parent");

        let query = Self::query(
            "
                (parent (class_type (class_name) @name)
                (feature_adaptation (rename (rename_pair (identifier) @rename_before
                        (extended_feature_name) @rename_after)* )?)?)
            ",
        );

        let mut matches = cursor.matches(&query, *node, src.as_bytes());
        let parent_match = matches.next().expect("parent captures.");

        let name = node_to_text(
            &capture_name_to_nodes("name", &query, parent_match)
                .next()
                .expect("capture class name."),
            src,
        )
        .to_string();

        let rename: Vec<(String, String)> =
            capture_name_to_nodes("rename_before", &query, parent_match)
                .zip(capture_name_to_nodes("rename_after", &query, parent_match))
                .map(|(before, after)| {
                    (
                        node_to_text(&before, src).to_string(),
                        node_to_text(&after, src).to_string(),
                    )
                })
                .collect();
        Ok(Parent {
            name,
            select: Vec::new(),
            rename,
            redefine: Vec::new(),
            undefine: Vec::new(),
        })
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::processed_file;
    use anyhow::Result;
    use std::fs::File;
    use std::io::prelude::*;
    use std::path::PathBuf;
    use tree_sitter;

    #[test]
    fn parse_base_class() {
        let src = "
    class A
    note
    end
        ";
        let class = Class::from_source(src);

        assert_eq!(
            class.name(),
            "A".to_string(),
            "Equality of {} and {}",
            class.name(),
            "A".to_string()
        );
    }

    #[test]
    fn parse_annotated_class() {
        let src = "
note
  demo_note: True
  multi_note: True, False
class DEMO_CLASS
invariant
  note
    note_after_invariant: True
end
    ";
        let class = Class::from_source(src);
        assert_eq!(class.name(), "DEMO_CLASS".to_string());
    }
    #[test]
    fn parse_procedure() {
        let src = "
class A feature
  f(x, y: INTEGER; z: BOOLEAN)
    do
    end
end
";
        let class = Class::from_source(src);
        assert_eq!(class.name(), "A".to_string());
        eprintln!("{class:?}");
        assert_eq!(class.features().first().unwrap().name(), "f".to_string());
    }

    #[test]
    fn parse_attribute() {
        let src = "
class A
feature
    x: INTEGER
end
";
        let class = Class::from_source(src);
        assert_eq!(class.name(), "A".to_string());
        eprintln!("{class:?}");
        assert_eq!(class.features().first().unwrap().name(), "x".to_string());
    }
    #[test]
    fn parse_model() {
        let src = "
note
    model: seq
class A
feature
    x: INTEGER
    seq: MML_SEQUENCE [INTEGER]
end
";
        let class = Class::from_source(src);
        assert_eq!(class.name(), "A".to_string());
        assert_eq!(
            class
                .features()
                .first()
                .expect("Parsed first feature")
                .name(),
            "x".to_string()
        );
        assert_eq!(
            (class.model().0.first().expect("Parsed model")).name(),
            "seq".to_string()
        );
    }
    #[test]
    fn parse_ancestors_names() {
        let src = "
class A
inherit {NONE}
  X Y Z

inherit
  W
end
";
        let class = Class::from_source(src);
        let mut ancestors = class.parents().into_iter();

        assert_eq!(class.name(), "A".to_string());

        assert_eq!(
            ancestors
                .next()
                .expect("fails to parse first ancestor")
                .name(),
            "X".to_string()
        );
        assert_eq!(
            ancestors
                .next()
                .expect("fails to parse second ancestor")
                .name(),
            "Y".to_string()
        );
        assert_eq!(
            ancestors
                .next()
                .expect("fails to parse third ancestor")
                .name(),
            "Z".to_string()
        );
        assert_eq!(
            ancestors
                .next()
                .expect("fails to parse forth ancestor")
                .name(),
            "W".to_string()
        );
    }
    #[test]
    fn parse_ancestors_renames() {
        let src = "
class A
inherit
  W
    rename e as f
end
";
        let class = Class::from_source(src);
        let mut ancestors = class.parents().into_iter();

        assert_eq!(
            ancestors.next().expect("fails to parse first ancestor"),
            &Parent {
                name: "W".to_string(),
                select: Vec::new(),
                rename: vec![("e".to_string(), "f".to_string())],
                redefine: Vec::new(),
                undefine: Vec::new()
            }
        );
    }

    #[test]
    fn rename_inherit_features() {
        let child_src = "
class A
inherit
  B
    rename y as z
end
";
        let parent_src = "
class B
inherit
  C
    rename x as y
end
";
        let grandparent_src = "
class C
feature
    x: BOOLEAN
end
";
        let grandparent = Class::from_source(grandparent_src);
        let parent = Class::from_source(parent_src);
        let child = Class::from_source(child_src);
        let system_classes = vec![&grandparent, &parent, &child];
        let child_features = child.inhereted_features(&system_classes);
        let parent_features = parent.inhereted_features(&system_classes);
        assert_eq!(
            grandparent.features().first().unwrap().name(),
            "x",
            "grandparent features"
        );
        assert_eq!(
            parent_features.first().unwrap().name(),
            "y",
            "parent features"
        );
        assert_eq!(
            child_features.first().unwrap().name(),
            "z",
            "child features"
        );
    }

    #[tokio::test]
    async fn processed_file_class_to_workspacesymbol() -> Result<()> {
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
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        let Some(file) = processed_file::ProcessedFile::new(&mut parser, path.clone()).await else {
            return Err(anyhow!("fails to process file"));
        };
        let symbol: Result<lsp_types::WorkspaceSymbol, _> = (&file).try_into();
        assert!(symbol.is_ok());
        Ok(())
    }
    #[test]
    fn parse_parent_classes() {
        let src_child = "
    class A
    inherit {NONE}
      X Y Z

    inherit
      W
        undefine a
        redefine c
        rename e as f
        export
          {ANY}
            -- Header comment
            all
        select g
        end
    end
    ";
        let src_parent = "
    note
        model: seq
    class W
    feature
        x: INTEGER
        seq: MML_SEQUENCE [INTEGER]
    end
    ";
        let child = Class::from_source(src_child);
        let parent = Class::from_source(src_parent);
        let system_classes = vec![&child, &parent];
        let child_parents = child.parent_classes(&system_classes).collect::<Vec<_>>();
        assert_eq!(child_parents, vec![&parent]);
    }

    #[test]
    fn ancestor_classes() {
        let child_src = "
class A
inherit
  B
    rename y as z
end
";
        let parent_src = "
class B
inherit
  C
    rename x as y
end
";
        let grandparent_src = "
class C
feature
    x: BOOLEAN
end
";
        let grandparent = Class::from_source(grandparent_src);
        let parent = Class::from_source(parent_src);
        let child = Class::from_source(child_src);

        let system_classes = vec![&child, &parent, &grandparent];

        let mut child_ancestors = HashSet::new();
        child_ancestors.insert(&parent);
        child_ancestors.insert(&grandparent);
        assert_eq!(child.ancestor_classes(&system_classes), child_ancestors);

        let mut parent_ancestors = HashSet::new();
        parent_ancestors.insert(&grandparent);
        assert_eq!(parent.ancestor_classes(&system_classes), parent_ancestors,);
    }
    #[test]
    fn full_model() -> Result<()> {
        let src_child = "
    note
        model: seq_child
    class A
    inherit
      W
    feature seq_child: MML_SEQUENCE[G]
    end
    ";
        let src_parent = "
    note
        model: seq_parent
    class W
    feature
        x: INTEGER
        seq_parent: MML_SEQUENCE [INTEGER]
    end
    ";
        let child = Class::from_source(src_child);
        let parent = Class::from_source(src_parent);
        assert_eq!(
            child.full_model(&vec![&child, &parent]).collect::<Vec<_>>(),
            vec![parent.model(), child.model()]
        );
        Ok(())
    }
    #[test]
    fn parameters_models() {
        let current_class = r#"class
    CLIENT
feature
    demo (a: NEW_INTEGER): INTEGER
        do
            a.value
        end
"#;
        let current_class = Class::from_source(current_class);
        let src_class_of_argument = r#"note
	model: value
class
	NEW_INTEGER
feature
	value: INTEGER
	smaller (other: NEW_INTEGER): BOOLEAN
		do
			Result := value < other.value
		ensure
			Result = (value < other.value)
		end
end
    "#;
        let class_of_argument = Class::from_source(src_class_of_argument);
        let model = class_of_argument.model();
        eprintln!("clas_of_argument {class_of_argument:#?}");
        assert_eq!(format!("{model}"), "value: INTEGER", "model: {model}");

        let system_classes = vec![&current_class, &class_of_argument];
        let feature = current_class
            .features()
            .first()
            .expect("demo is the first feature.");
        let mut parameter_model = feature
            .parameters()
            .full_model(&system_classes)
            .next()
            .expect("parameter has model.");

        assert_eq!(
            parameter_model.0, "a",
            "parameter name: {}",
            parameter_model.0
        );
        let parameter_model = parameter_model.1.next().expect("`NEW_INTEGER` has model");
        assert_eq!(
            format!("{}", parameter_model),
            "value: INTEGER",
            "parameter name: {parameter_model}",
        );
    }
}
