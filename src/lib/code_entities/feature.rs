use super::prelude::*;
use async_lsp::lsp_types;
use contract::RoutineSpecification;
use contract::{Block, Postcondition, Precondition};
use std::fmt::Display;
use std::path::Path;
use streaming_iterator::StreamingIterator;

mod notes;
pub use notes::Notes;

mod eiffel_type;
pub use eiffel_type::EiffelType;

mod parameters;
pub use parameters::Parameters;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum FeatureVisibility {
    Private,
    Some(Box<Class>),
    Public,
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Feature {
    pub name: String,
    pub parameters: Parameters,
    pub return_type: Option<EiffelType>,
    pub notes: Option<Notes>,
    pub visibility: FeatureVisibility,
    pub range: Range,
    /// Is None only when a precondition cannot be added (for attributes without an attribute clause).
    pub preconditions: Option<Block<Precondition>>,
    pub postconditions: Option<Block<Postcondition>>,
}
impl Feature {
    pub fn is_feature_around_point(&self, point: Point) -> bool {
        point >= self.range().start && point <= self.range().end
    }
    pub fn feature_around_point<'feature>(
        mut features: impl Iterator<Item = &'feature Feature>,
        point: Point,
    ) -> Option<&'feature Feature> {
        features.find(|f| f.is_feature_around_point(point))
    }
    pub fn clone_rename(&self, name: String) -> Feature {
        let mut f = self.clone();
        f.name = name;
        f
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn parameters(&self) -> &Parameters {
        &self.parameters
    }
    pub fn number_parameters(&self) -> usize {
        let parameters = self.parameters();
        debug_assert_eq!(parameters.names().len(), parameters.types().len());
        parameters.names().len()
    }
    pub fn return_type(&self) -> Option<&EiffelType> {
        self.return_type.as_ref()
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
    pub fn preconditions(&self) -> Option<&Precondition> {
        self.preconditions.as_ref().map(|b| b.item())
    }
    pub fn postconditions(&self) -> Option<&Postcondition> {
        self.postconditions.as_ref().map(|b| b.item())
    }
    pub fn routine_specification(&self) -> RoutineSpecification {
        let postcondition = self.postconditions().cloned().unwrap_or_default();
        let precondition = self.preconditions().cloned().unwrap_or_default();
        RoutineSpecification {
            precondition,
            postcondition,
        }
    }
    pub fn has_precondition(&self) -> bool {
        self.preconditions().is_some_and(|p| !p.is_empty())
    }
    pub fn has_postcondition(&self) -> bool {
        self.postconditions().is_some_and(|p| !p.is_empty())
    }
    pub fn point_end_preconditions(&self) -> Option<Point> {
        match &self.preconditions {
            Some(pre) => Some(pre.range().end),
            None => return None,
        }
    }
    pub fn point_start_preconditions(&self) -> Option<Point> {
        match &self.preconditions {
            Some(pre) => Some(pre.range().start),
            None => return None,
        }
    }
    pub fn point_end_postconditions(&self) -> Option<Point> {
        match &self.postconditions {
            Some(post) => Some(post.range().end),
            None => None,
        }
    }
    pub fn point_start_postconditions(&self) -> Option<Point> {
        match &self.postconditions {
            Some(post) => Some(post.range().start),
            None => None,
        }
    }
    pub fn supports_precondition_block(&self) -> bool {
        self.preconditions.is_some()
    }
    pub fn supports_postcondition_block(&self) -> bool {
        self.postconditions.is_some()
    }
    pub async fn src_unchecked<'src>(&self, path: &Path) -> anyhow::Result<String> {
        let range = self.range();
        let start_column = range.start.column;
        let start_row = range.start.row;
        let end_column = range.end.column;
        let end_row = range.end.row;

        let file_source = String::from_utf8(tokio::fs::read(&path).await?)?;
        let feature = file_source
            .lines()
            .skip(start_row)
            .enumerate()
            .map_while(|(linenum, line)| match linenum {
                0 => Some(&line[start_column..]),
                n if n < end_row - start_row => Some(line),
                n if n == end_row - start_row => Some(&line[..end_column]),
                _ => None,
            })
            .fold(String::new(), |mut acc, line| {
                acc.push_str(line);
                acc.push('\n');
                acc
            });
        Ok(feature)
    }
}
impl Display for Feature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = self.name();
        let parenthesized_parameters = if self.parameters().is_empty() {
            String::new()
        } else {
            format!("({})", self.parameters())
        };
        let format_return_type = self.return_type().map_or_else(
            || String::new(),
            |ref return_type| format!(": {return_type}"),
        );
        write!(f, "{name}{parenthesized_parameters}{format_return_type}")
    }
}
impl Indent for Feature {
    const INDENTATION_LEVEL: usize = 1;
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
    use anyhow::Context;

    use super::*;
    use crate::lib::parser::Parser;

    fn class(src: &str) -> anyhow::Result<Class> {
        let mut parser = Parser::new();
        parser.class_from_source(src)
    }

    #[test]
    fn parse_feature_with_precondition() -> anyhow::Result<()> {
        let src = r#"
class A feature
  x
    require
      True
    do
    end

  y
    require else
      True
    do
    end
end"#;
        let class = class(src)?;
        eprintln!("{class:?}");
        let feature = class.features().first().expect("first features is `x`");

        assert_eq!(feature.name(), "x");
        assert!(feature.preconditions().is_some());
        assert!(feature.preconditions().unwrap().first().is_some());

        let predicate = &feature.preconditions().unwrap().first().unwrap().predicate;
        assert_eq!(predicate.as_str(), "True");
        Ok(())
    }

    #[test]
    fn eiffel_type_class_name() {
        let eiffeltype = EiffelType::ClassType(
            "MML_SEQUENCE [INTEGER]".to_string(),
            "MML_SEQUENCE".to_string(),
        );
        assert!(eiffeltype
            .class_name()
            .is_ok_and(|name| name == *"MML_SEQUENCE"));
    }
}
