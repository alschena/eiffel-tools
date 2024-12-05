use super::prelude::*;
use crate::lib::tree_sitter_extension::Parse;
use async_lsp::lsp_types;
use contract::{Block, Postcondition, Precondition};
use std::borrow::Cow;
use std::fmt::Display;
use streaming_iterator::StreamingIterator;
use tracing::instrument;
use tree_sitter::{Node, Query, QueryCursor};
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum FeatureVisibility {
    Private,
    Some(Box<Class>),
    Public,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
struct Parameters(Vec<(String, String)>);
impl Parameters {
    fn add_parameter(&mut self, id: String, eiffel_type: String) {
        self.0.push((id, eiffel_type));
    }
}
impl Parse for Parameters {
    type Error = anyhow::Error;

    fn parse(node: &Node, src: &str) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "formal_arguments");

        let mut cursor = QueryCursor::new();
        let lang = &tree_sitter_eiffel::LANGUAGE.into();

        let entity_declaration_group_query =
            Query::new(lang, "(entity_declaration_group) @declarationgroup")
                .expect("Query for `entity_declaration_group` nodes must succeed.");

        let mut entity_declaration_group_cursor = cursor.matches(
            &entity_declaration_group_query,
            node.clone(),
            src.as_bytes(),
        );
        let mut parameters = Parameters(Vec::new());
        while let Some(entity_declaration_match) = entity_declaration_group_cursor.next() {
            for entity_declaration_capture in entity_declaration_match.captures {
                let mut query_cursor = QueryCursor::new();
                let node = entity_declaration_capture.node;
                let parameter_name_query = Query::new(lang, "(identifier) @parameter_name")
                    .expect("Query parameter's name for a certain entity declaration block.");
                let parameter_type_query = Query::new(lang, "(class_type) @parameter_type")
                    .expect("Query parameter's type for a certain entity declaration block.");

                let parameter_type: String = src[query_cursor
                    .matches(&parameter_type_query, node.clone(), src.as_bytes())
                    .next()
                    .expect("There must be a type for each entity declaration match.")
                    .captures[0]
                    .node
                    .byte_range()]
                .into();
                query_cursor
                    .matches(&parameter_name_query, node.clone(), src.as_bytes())
                    .for_each(|mat| {
                        mat.captures.iter().for_each(|cap| {
                            parameters.add_parameter(
                                src[cap.node.byte_range()].into(),
                                parameter_type.clone(),
                            )
                        })
                    });
            }
        }
        Ok(parameters)
    }
}
impl Display for Parameters {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = self.0.iter().fold(
            String::new(),
            |mut acc, (parameter_name, parameter_type)| {
                acc.push_str(parameter_name.as_str());
                acc.push_str(": ");
                acc.push_str(parameter_type.as_str());
                acc
            },
        );
        write!(f, "{text}");
        Ok(())
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Feature {
    pub(super) name: String,
    parameters: Parameters,
    return_type: String,
    pub(super) visibility: FeatureVisibility,
    pub(super) range: Range,
    /// Is None only when a precondition cannot be added (for attributes without an attribute clause).
    preconditions: Option<Block<Precondition>>,
    postconditions: Option<Block<Postcondition>>,
}
impl Feature {
    pub fn name(&self) -> &str {
        &self.name
    }
    fn parameters(&self) -> &Parameters {
        &self.parameters
    }
    fn return_type(&self) -> &str {
        &self.return_type
    }
    fn signature(&self) -> String {
        format!("{} ({}): {}", self.name, self.parameters, self.return_type)
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
    pub fn preconditions(&self) -> Option<&Precondition> {
        match &self.preconditions {
            Some(pre) => pre.item(),
            None => None,
        }
    }
    pub fn postconditions(&self) -> Option<&Postcondition> {
        match &self.postconditions {
            Some(post) => post.item(),
            None => None,
        }
    }
    pub fn has_precondition(&self) -> bool {
        self.preconditions().is_some()
    }
    pub fn has_postcondition(&self) -> bool {
        self.postconditions().is_some()
    }
    pub fn point_end_preconditions(&self) -> Option<&Point> {
        match &self.preconditions {
            Some(pre) => Some(pre.range().end()),
            None => return None,
        }
    }
    pub fn point_start_preconditions(&self) -> Option<&Point> {
        match &self.preconditions {
            Some(pre) => Some(pre.range().start()),
            None => return None,
        }
    }
    pub fn point_end_postconditions(&self) -> Option<&Point> {
        match &self.postconditions {
            Some(post) => Some(post.range().end()),
            None => None,
        }
    }
    pub fn point_start_postconditions(&self) -> Option<&Point> {
        match &self.postconditions {
            Some(post) => Some(post.range().start()),
            None => None,
        }
    }
    pub fn supports_precondition_block(&self) -> bool {
        self.preconditions.is_some()
    }
    pub fn supports_postcondition_block(&self) -> bool {
        self.postconditions.is_some()
    }
}
impl Indent for Feature {
    const INDENTATION_LEVEL: u32 = 1;
}
impl Parse for Feature {
    type Error = anyhow::Error;
    #[instrument(skip_all)]
    fn parse(node: &Node, src: &str) -> anyhow::Result<Self> {
        debug_assert!(node.kind() == "feature_declaration");

        let mut query_cursor = QueryCursor::new();
        let lang = &tree_sitter_eiffel::LANGUAGE.into();

        let name_query = Query::new(lang, r#"(extended_feature_name) @name"#)
            .expect("Query for `extended_feature_name` must succeed.");
        let mut name_captures = query_cursor.captures(&name_query, node.clone(), src.as_bytes());
        let name = src[name_captures.next().expect("Should have name").0.captures[0]
            .node
            .byte_range()]
        .into();

        let parameters_query = Query::new(lang, "(formal_arguments) @parameters")
            .expect("Query for `formal_arguments` of the feature must succeed.");
        let parameters = query_cursor
            .captures(&parameters_query, node.clone(), src.as_bytes())
            .next()
            .map_or_else(
                || Ok(Parameters(Vec::new())),
                |formal_arguments| Parameters::parse(&formal_arguments.0.captures[0].node, src),
            )?;

        let return_type_query = Query::new(
            lang,
            "(feature_declaration (class_type (class_name) @return_type))",
        )
        .expect("Query for the return type of the feature must succeed.");
        let return_type = query_cursor
            .captures(&return_type_query, node.clone(), src.as_bytes())
            .next()
            .map_or_else(
                || String::new(),
                |return_type| src[return_type.0.captures[0].node.byte_range()].into(),
            );

        let attribute_or_routine_captures_query =
            Query::new(lang, "(attribute_or_routine) @x").unwrap();
        let mut attribute_or_routine_captures = query_cursor.captures(
            &attribute_or_routine_captures_query,
            node.clone(),
            src.as_bytes(),
        );
        let aor = attribute_or_routine_captures.next();
        let preconditions = match aor {
            Some(x) => Some(Block::<Precondition>::parse(&x.0.captures[0].node, src)?),
            None => None,
        };
        let postconditions = match aor {
            Some(x) => Some(Block::<Postcondition>::parse(&x.0.captures[0].node, src)?),
            None => None,
        };

        Ok(Feature {
            name,
            visibility: FeatureVisibility::Private,
            range: node.range().into(),
            parameters,
            return_type,
            preconditions,
            postconditions,
        })
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
    use crate::lib::tree_sitter_extension::WidthFirstTraversal;

    use super::*;

    #[test]
    fn parse_feature_with_precondition() {
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
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();

        let lang = &tree_sitter_eiffel::LANGUAGE.into();
        let query = ::tree_sitter::Query::new(lang, "(feature_declaration) @name").unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());
        let node = captures.next().unwrap().0.captures[0].node;

        let feature = Feature::parse(&node, &src).expect("Parse feature");
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
