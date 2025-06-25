use std::sync::LazyLock;

use anyhow::Context;
use anyhow::Result;

use crate::code_entities::prelude::ClassParent;
use crate::code_entities::prelude::FeatureName;
use crate::parser::Node;
use crate::parser::Query;
use crate::parser::util;
use crate::parser::util::Traversal;

pub static INHERITANCE_QUERY: LazyLock<Query> = LazyLock::new(|| {
    util::query(
        r#"
                (parent (class_type (class_name) @name)
                (feature_adaptation (rename (rename_pair (identifier) @rename_before
                        (extended_feature_name) @rename_after)* )?)?)
            "#,
    )
});

pub trait InheritanceTree<'source, 'tree> {
    fn goto_inheritance_tree(&mut self, parent_node: Node<'tree>);
    fn parent(&mut self) -> Result<ClassParent>;
}

impl<'source, 'tree, T> InheritanceTree<'source, 'tree> for T
where
    T: Traversal<'source, 'tree>,
{
    fn goto_inheritance_tree(&mut self, parent_node: Node<'tree>) {
        assert_eq!(parent_node.kind(), "parent");
        self.set_node_and_query(parent_node, &INHERITANCE_QUERY);
    }

    fn parent(&mut self) -> Result<ClassParent> {
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
            .map(|(&before, &after)| -> Result<_> {
                Ok((
                    FeatureName::new(self.node_content(before)?),
                    FeatureName::new(self.node_content(after)?),
                ))
            })
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
