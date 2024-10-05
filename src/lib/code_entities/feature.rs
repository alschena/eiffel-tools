use super::class::Class;
use super::*;
use crate::lib::tree_sitter::{self, ExtractedFrom};
use anyhow::anyhow;
use async_lsp::lsp_types;
use contract::PreconditionDecorated;
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum FeatureVisibility {
    Private,
    Some(Box<Class>),
    Public,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct Feature {
    pub(super) name: String,
    pub(super) visibility: FeatureVisibility,
    pub(super) range: Range,
    pub(super) preconditions: Option<PreconditionDecorated>,
    pub(super) postconditions: Option<Postcondition>,
}
impl Feature {
    pub fn from_name_and_range(name: String, range: Range) -> Feature {
        Feature {
            name,
            visibility: FeatureVisibility::Private,
            range,
            preconditions: None,
            postconditions: None,
        }
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
    pub fn preconditions(&self) -> &Option<PreconditionDecorated> {
        &self.preconditions
    }
    pub fn range_end_preconditions(&self) -> &Range {
        match &self.preconditions {
            Some(precondition) => precondition.range(),
            None => todo!(),
        }
    }
}
impl ExtractedFrom for Feature {
    type Error = anyhow::Error;
    fn extract_from(cursor: &mut tree_sitter::TreeCursor, src: &str) -> anyhow::Result<Self> {
        debug_assert!(cursor.node().kind() == "feature_declaration");
        let node = cursor.node();
        let name = src[tree_sitter::WidthFirstTraversal::new(cursor)
            .find(|x| x.kind() == "extended_feature_name")
            .ok_or(anyhow!(
                "Each feature declaration contains an extended feature name"
            ))?
            .byte_range()]
        .into();
        cursor.reset(node);
        let preconditions = match tree_sitter::WidthFirstTraversal::new(cursor)
            .find(|attribute_or_routine| attribute_or_routine.kind() == "attribute_or_routine")
        {
            Some(attribute_or_routine) => {
                cursor.reset(attribute_or_routine);
                Some(PreconditionDecorated::extract_from(cursor, src)?)
            }
            None => None,
        };
        Ok(Feature {
            name,
            visibility: FeatureVisibility::Private,
            range: node.range().into(),
            preconditions,
            postconditions: None,
        })
    }
}
impl TryFrom<lsp_types::DocumentSymbol> for Feature {
    type Error = anyhow::Error;

    fn try_from(value: lsp_types::DocumentSymbol) -> std::result::Result<Self, Self::Error> {
        let name = value.name;
        let kind = value.kind;
        let range = value.range.try_into()?;
        debug_assert_ne!(kind, lsp_types::SymbolKind::CLASS);
        Ok(Feature::from_name_and_range(name, range))
    }
}
impl TryFrom<&Feature> for lsp_types::DocumentSymbol {
    type Error = anyhow::Error;

    fn try_from(value: &Feature) -> std::result::Result<Self, Self::Error> {
        let name = value.name().to_string();
        let range = value.range().clone().try_into()?;
        Ok(lsp_types::DocumentSymbol {
            name,
            detail: None,
            kind: lsp_types::SymbolKind::METHOD,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children: None,
        })
    }
}
#[cfg(test)]
mod tests {
    use crate::lib::tree_sitter::WidthFirstTraversal;

    use super::*;

    #[test]
    fn extract_feature_with_precondition() {
        let src = r#"
class A feature
  x
    require
      True
    do
    end

  y
    require else
    do
    end
end"#;
        let mut parser = ::tree_sitter::Parser::new();
        parser
            .set_language(tree_sitter_eiffel::language())
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();
        let mut cursor = tree.walk();
        let node = WidthFirstTraversal::new(&mut cursor)
            .find(|node| node.kind() == "feature_declaration")
            .expect("feature declaration");
        cursor.reset(node);
        let feature = Feature::extract_from(&mut cursor, &src).expect("Parse feature");
        assert_eq!(feature.name(), "x");
        let predicate = feature
            .preconditions()
            .clone()
            .expect("extracted preconditions")
            .precondition
            .first()
            .expect("non empty precondition")
            .predicate
            .clone()
            .predicate;
        assert_eq!(predicate, "True".to_string())
    }
}
