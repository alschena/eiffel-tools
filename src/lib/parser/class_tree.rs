use crate::lib::parser::util::Traversal;
use crate::lib::parser::*;
use anyhow::ensure;
use anyhow::Result;

mod contract_tree;
mod eiffel_type;
mod feature_tree;

use feature_tree::FeatureClauseTree;
pub use feature_tree::FeatureTree;

mod inheritance_tree;
use inheritance_tree::InheritanceTree;
use notes_tree::NotesTree;

mod notes_tree;

pub(super) fn query() -> Query {
    util::query(
        r#"
            (class_declaration
                (notes)? @notes
                (class_name) @name
                (inheritance (parent)* @parent)* @inheritance
                (feature_clause)* @feature_clause
                (notes)? @notes
            )@class
                
            "#,
    )
}

struct ClassDeclarationNodes<'tree> {
    notes_nodes: Vec<Node<'tree>>,
    name_node: Node<'tree>,
    // TODO: change to inheritance nodes and capture non-confomance
    parents_nodes: Vec<Node<'tree>>,
    feature_clause_nodes: Vec<Node<'tree>>,
    class_node: Node<'tree>,
}

impl<'source, 'tree> ClassDeclarationNodes<'tree> {
    fn range(&self) -> Range {
        self.class_node.range().into()
    }
}

impl<'source, 'tree> TreeTraversal<'source, 'tree> {
    fn class_name(&self, nodes: &ClassDeclarationNodes) -> Result<ClassName> {
        self.node_content(nodes.name_node)
            .map(|name| ClassName(name.into()))
    }

    // TODO: iterate on inheritance nodes instead of parents nodes to capture non-conformance.
    fn class_parents(&mut self, nodes: &ClassDeclarationNodes<'tree>) -> Result<Vec<ClassParent>> {
        let mut parents = Vec::new();
        for node in &nodes.parents_nodes {
            self.goto_inheritance_tree(*node);
            parents.push(self.parent()?);
        }
        Ok(parents)
    }

    fn class_feature_by_clauses(
        &mut self,
        nodes: &ClassDeclarationNodes<'tree>,
    ) -> Result<Vec<Vec<Feature>>> {
        let mut features_by_clause = Vec::new();
        for node in &nodes.feature_clause_nodes {
            self.goto_feature_clause_tree(*node);
            features_by_clause.push(self.features()?);
        }
        Ok(features_by_clause)
    }

    fn class_model_names(&mut self, nodes: &ClassDeclarationNodes<'tree>) -> Result<ModelNames> {
        let mut model_names = ModelNames::new(Vec::new());
        for node in &nodes.notes_nodes {
            self.goto_notes_tree(*node);
            model_names.extend(self.model_names()?.iter().map(|name| name.to_string()));
        }
        Ok(model_names)
    }

    pub(super) fn class(&mut self) -> Result<Class> {
        let nodes: ClassDeclarationNodes<'tree> = self.try_into()?;
        let name = self.class_name(&nodes)?;
        let features = self.class_feature_by_clauses(&nodes)?;
        let features = features
            .into_iter()
            .reduce(|mut acc, mut features_in_clause| {
                acc.append(&mut features_in_clause);
                acc
            })
            .unwrap_or_default();
        let model = ClassLocalModel::try_from_names_and_features(
            self.class_model_names(&nodes)?,
            &features,
        )?;
        let parents = self.class_parents(&nodes)?;
        let range = nodes.range();
        Ok(Class {
            name,
            model,
            features,
            parents,
            range,
        })
    }
}

impl<'source, 'tree> TryFrom<&mut TreeTraversal<'source, 'tree>> for ClassDeclarationNodes<'tree> {
    type Error = anyhow::Error;

    fn try_from(
        value: &mut TreeTraversal<'source, 'tree>,
    ) -> std::result::Result<Self, Self::Error> {
        ensure!(value.current_node().kind() == "source_file");

        let notes_nodes = value.nodes_captures("notes")?;
        let name_node = *value
            .nodes_captures("name")?
            .first()
            .with_context(|| "fails to get class name node")?;
        let parents_nodes = value.nodes_captures("parent")?;
        let feature_clause_nodes = value.nodes_captures("feature_clause")?;
        let class_node = *value
            .nodes_captures("class")?
            .first()
            .with_context(|| "fails to get class name node")?;

        Ok(Self {
            notes_nodes,
            name_node,
            parents_nodes,
            feature_clause_nodes,
            class_node,
        })
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::lib::parser::tests::EMPTY_CLASS;
    use anyhow::anyhow;

    /// class
    ///     TEST
    /// feature
    ///     x: INTEGER
    ///     y: INTEGER
    /// end
    pub const DOUBLE_ATTRIBUTE_CLASS: &str = r#"
class
    TEST
feature
    x: INTEGER
    y: INTEGER
end
"#;

    pub const MODEL_CLASS: &str = r#"
note
    model: x
class
    MODEL_CLASS
feature
    x: INTEGER
end
"#;

    pub const PARENT_CLASS: &str = r#"
class A
inherit {NONE}
  X Y Z

inherit
  W
end
"#;

    pub const RENAME_PARENT_CLASS: &str = r#"
class A
inherit
  W
    rename e as f
end
"#;

    pub const SEQ_MODEL_CLASS: &str = r#"
note
    model: seq
class A
feature
    x: INTEGER
    seq: MML_SEQUENCE [INTEGER]
end
"#;

    pub const ANNOTATED_CLASS: &str = r#"
note
  demo_note: True
  multi_note: True, False
class DEMO_CLASS
invariant
  note
    note_after_invariant: True
end
    "#;

    pub const PROCEDURE_CLASS: &str = r#"
class A feature
  f(x, y: INTEGER; z: BOOLEAN)
    do
    end
end"#;

    pub const FUNCTION_CLASS: &str = r#"
class A feature
  x (y, z: MML_SEQUENCE [INTEGER]): MML_SEQUENCE [INTEGER]
    do
    end
end"#;

    #[test]
    fn class_declaration_nodes() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_source = parser.parse(DOUBLE_ATTRIBUTE_CLASS)?;
        let mut class_tree = parsed_source.class_tree_traversal()?;

        let ClassDeclarationNodes {
            notes_nodes,
            name_node,
            parents_nodes,
            feature_clause_nodes,
            class_node: _,
        } = (&mut class_tree).try_into()?;

        assert!(
            notes_nodes.is_empty(),
            "There is no note block in {}",
            DOUBLE_ATTRIBUTE_CLASS
        );

        assert_eq!(class_tree.node_content(name_node)?, "TEST");

        assert_eq!(
            feature_clause_nodes.len(),
            1,
            "fails to parse the single feature clause, i.e. feature visibility block."
        );
        assert_eq!(
            class_tree.node_content(*feature_clause_nodes.first().unwrap())?,
            r#"feature
    x: INTEGER
    y: INTEGER
"#,
            "fails to parse the single feature clause, i.e. feature visibility block."
        );
        assert!(parents_nodes.is_empty());
        Ok(())
    }

    #[test]
    fn empty_class() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(EMPTY_CLASS)?;
        let mut class_tree = parsed_file.class_tree_traversal()?;
        let class = class_tree.class()?;
        assert_eq!(
            class.name(),
            "A",
            "fails to parse class source: {EMPTY_CLASS}. Parsed class: {class:#?}"
        );
        Ok(())
    }

    #[test]
    fn procedure_class() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(PROCEDURE_CLASS)?;
        let mut class_tree = parsed_file.class_tree_traversal()?;
        let mut class = class_tree.class()?;
        let procedure = class
            .features
            .pop()
            .with_context(|| "fails to get procedure of class source: {PROCEDURE_CLASS}.")?;
        assert_eq!(procedure.name(), "f");
        Ok(())
    }

    #[test]
    fn double_attribute_class() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(DOUBLE_ATTRIBUTE_CLASS)?;
        let mut class_tree = parsed_file.class_tree_traversal()?;
        let mut class = class_tree.class()?;
        let second_feature = class
            .features
            .pop()
            .with_context(|| "fails to get second feature")?;
        let first_feature = class
            .features
            .pop()
            .with_context(|| "fails to get first feature")?;
        assert_eq!(first_feature.name(), "x");
        assert_eq!(second_feature.name(), "y");
        Ok(())
    }

    #[test]
    fn model_class() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(MODEL_CLASS)?;
        let parsed_file2 = parser.parse(SEQ_MODEL_CLASS)?;
        let mut class_tree = parsed_file.class_tree_traversal()?;
        let mut class_tree2 = parsed_file2.class_tree_traversal()?;
        let class = class_tree.class()?;
        let class2 = class_tree2.class()?;
        let model = class.model;
        let model2 = class2.model;

        eprintln!("model: {model:#?}");
        assert_eq!(*model.names(), ModelNames::new(vec!["x".to_string()]));
        assert_eq!(
            model
                .types()
                .first()
                .map(|ty| ty.class_name().map_err(|e| anyhow!(
                    "fails to get class name from eiffel type with error: {e}"
                )))
                .transpose()?,
            Some(ClassName("INTEGER".to_string()))
        );

        eprintln!("model2: {model2:#?}");
        assert_eq!(*model2.names(), ModelNames::new(vec!["seq".to_string()]));
        assert_eq!(
            model2
                .types()
                .first()
                .map(|ty| ty.class_name().map_err(|e| anyhow!(
                    "fails to get class name from eiffel type with error: {e}"
                )))
                .transpose()?,
            Some(ClassName("MML_SEQUENCE".to_string()))
        );
        Ok(())
    }

    #[test]
    fn parents_class() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(PARENT_CLASS)?;
        let mut class_tree = parsed_file.class_tree_traversal()?;
        let class = class_tree.class()?;
        let parents = class.parents;
        let parent_names: Vec<_> = parents.iter().map(|parent| parent.name.clone()).collect();
        assert_eq!(
            parent_names,
            vec![
                "X".to_string(),
                "Y".to_string(),
                "Z".to_string(),
                "W".to_string()
            ],
            "parent_names: {parent_names:#?}"
        );
        Ok(())
    }

    #[test]
    fn rename_parent_class() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(RENAME_PARENT_CLASS)?;
        let mut class_tree = parsed_file.class_tree_traversal()?;
        let class = class_tree.class()?;
        let rename = class
            .parents
            .first()
            .map(|parent| &parent.rename)
            .with_context(|| {
                format!("fails to get parent from class source: {RENAME_PARENT_CLASS}")
            })?;
        let (before_name, after_name) = rename.first().with_context(|| {
            format!("fails to get rename from parent in sourcec {RENAME_PARENT_CLASS}")
        })?;
        assert_eq!(before_name, "e", "name before renaming.");
        assert_eq!(after_name, "f", "name after renaming.");
        Ok(())
    }
}
