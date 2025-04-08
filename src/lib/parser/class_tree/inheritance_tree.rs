use anyhow::Context;

use crate::lib::code_entities::prelude::ClassParent;
use crate::lib::parser::util;
use crate::lib::parser::util::Traversal;
use crate::lib::parser::Node;
use crate::lib::parser::Query;

pub trait InheritanceTree<'source, 'tree>: Traversal<'source, 'tree> {
    fn query() -> Query {
        util::query(
            r#"
                (parent (class_type (class_name) @name)
                (feature_adaptation (rename (rename_pair (identifier) @rename_before
                        (extended_feature_name) @rename_after)* )?)?)
            "#,
        )
    }

    fn goto_inheritance_tree(&mut self, parent_node: Node<'tree>) {
        assert_eq!(parent_node.kind(), "parent");
        self.set_node_and_query(parent_node, <Self as InheritanceTree>::query());
    }

    fn parent(&mut self) -> Result<ClassParent, Self::Error> {
        assert_eq!(
            self.current_node().kind(),
            "parent",
            "current node: {}",
            self.current_node()
        );

        let name = self
            .nodes_captures("name")?
            .first()
            .map(|&name_node| self.node_content(name_node))
            .with_context(|| "fails to get parent name.")??
            .to_string();

        let rename: Vec<_> = self
            .nodes_captures("rename_before")?
            .iter()
            .zip(self.nodes_captures("rename_after")?.iter())
            .map(
                |(&before, &after)| -> Result<(String, String), Self::Error> {
                    Ok((
                        self.node_content(before)?.to_string(),
                        self.node_content(after)?.to_string(),
                    ))
                },
            )
            .collect::<Result<Vec<_>, _>>()?;

        Ok(ClassParent {
            name,
            select: Vec::new(),
            rename,
            redefine: Vec::new(),
            undefine: Vec::new(),
        })
    }
}

impl<'source, 'tree, T: Traversal<'source, 'tree>> InheritanceTree<'source, 'tree> for T {}
