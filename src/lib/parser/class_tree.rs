use crate::lib::parser::util::Traversal;
use crate::lib::parser::*;
use anyhow::anyhow;
use anyhow::bail;
use anyhow::ensure;

mod contract_tree;
mod eiffel_type;
mod feature_tree;
use anyhow::Result;
use feature_tree::FeatureTree;
mod inheritance_tree;
use inheritance_tree::InheritanceTree;
mod notes_tree;

pub trait ClassTree<'source, 'tree>:
    FeatureTree<'source, 'tree> + InheritanceTree<'source, 'tree>
{
    fn query() -> Query {
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

    fn class(&mut self) -> Result<Class, Self::Error> {
        if self.current_node().kind() != "source_file" {
            return Err(anyhow!("class tree current node is root").into());
        }
        let name_nodes = self.class_name()?;
        let notes_nodes = self.class_notes()?;
        let parents_nodes = self.nodes_captures("parent")?;
        let features_clauses_nodes = self.feature_clauses()?;
        let range = self
            .nodes_captures("class")?
            .first()
            .map(|class_node| class_node.range())
            .with_context(|| "fails to get class declaration node.")?
            .into();

        let features = features_clauses_nodes
            .iter()
            .map(|&feature_clause_node| -> Result<_, _> {
                self.goto_feature_tree(feature_clause_node);
                self.features()
            })
            .fold(Ok(Vec::new()), |acc, features| {
                if let (Ok(mut acc), Ok(ref mut features)) = (acc, features) {
                    acc.append(features);
                    Ok(acc)
                } else {
                    bail!("fails to get features");
                }
            })?;

        let model = notes_nodes
            .iter()
            .map(|&note_node| {
                self.goto_notes_tree(note_node);
                self.model_names().map(|names| {
                    names
                        .iter()
                        .map(|name| name.to_string())
                        .collect::<Vec<String>>()
                })
            })
            .fold(Ok(Vec::new()), |acc, model_names| {
                if let (Ok(mut acc), Ok(ref mut model_names)) = (acc, model_names) {
                    acc.append(model_names);
                    Ok(acc)
                } else {
                    bail!("fails to get model names.");
                }
            })?
            .into_iter()
            .map(|model_name| -> Result<_, _> {
                let ft = features
                    .iter()
                    .find(|ft| ft.name() == model_name)
                    .with_context(|| "model feature not found {model_name:#?}")?;
                let model_type = ft
                    .return_type()
                    .with_context(|| "model feature {ft:#?} must have a return type.")?
                    .clone();

                Ok((model_name, model_type))
            })
            .collect::<Result<(Vec<String>, Vec<EiffelType>), _>>()
            .map(|(names, types)| {
                ClassLocalModel(ModelNames::new(names), ModelTypes::new(types))
            })?;

        let parents = parents_nodes
            .iter()
            .map(|&parent_node| -> Result<_, _> {
                self.goto_inheritance_tree(parent_node);
                self.parent()
            })
            .collect::<Result<Vec<_>, _>>()?;

        Ok(Class {
            name: ClassName(String::from(self.node_content(name_nodes)?)),
            model,
            features,
            parents,
            range,
        })
    }

    fn class_name(&mut self) -> Result<Node<'tree>, Self::Error> {
        let mut nodes = self.nodes_captures("name")?;
        Ok(nodes.pop().with_context(|| "TOOO")?)
    }

    fn class_notes(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("notes")
    }

    fn inheritance(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("inheritance")
    }

    fn feature_clauses(&mut self) -> Result<Vec<Node<'tree>>, Self::Error> {
        self.nodes_captures("feature_clause")
    }
}

impl<'source, 'tree, T> ClassTree<'source, 'tree> for T where T: Traversal<'source, 'tree> {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::parser::tests::EMPTY_CLASS;
    use crate::lib::parser::util::TreeTraversal;
    use anyhow::anyhow;

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

    #[test]
    fn class_name_node() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(DOUBLE_ATTRIBUTE_CLASS)?;
        let mut class_tree = TreeTraversal::try_from(&parsed_file)?;
        let name = class_tree.class_name()?;
        let content = class_tree.node_content(name)?;
        assert_eq!(content, "TEST");
        Ok(())
    }

    #[test]
    fn class_feature_nodes() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(DOUBLE_ATTRIBUTE_CLASS)?;
        let mut class_tree = TreeTraversal::try_from(&parsed_file)?;

        let mut features_clause = class_tree.feature_clauses()?;
        assert_eq!(
            features_clause.len(),
            1,
            "fails to parse the single feature clause, i.e. feature visibility block."
        );
        let feature_clause_content = class_tree.node_content(features_clause.pop().unwrap())?;
        assert_eq!(
            feature_clause_content.trim(),
            r#"feature
    x: INTEGER
    y: INTEGER"#
        );
        Ok(())
    }

    #[test]
    fn inheritance_node() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(DOUBLE_ATTRIBUTE_CLASS)?;
        let mut class_tree = TreeTraversal::try_from(&parsed_file)?;

        let inheritance_tree = class_tree.inheritance()?;
        assert!(inheritance_tree.is_empty());
        Ok(())
    }

    #[test]
    fn empty_class() -> anyhow::Result<()> {
        let mut parser = Parser::new();
        let parsed_file = parser.parse(EMPTY_CLASS)?;
        let mut class_tree = TreeTraversal::try_from(&parsed_file)?;
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
        let mut class_tree = TreeTraversal::try_from(&parsed_file)?;
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
        let mut class_tree = TreeTraversal::try_from(&parsed_file)?;
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
        let mut class_tree = TreeTraversal::try_from(&parsed_file)?;
        let mut class_tree2 = TreeTraversal::try_from(&parsed_file2)?;
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
        let mut class_tree = TreeTraversal::try_from(&parsed_file)?;
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
        let mut class_tree = TreeTraversal::try_from(&parsed_file)?;
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
