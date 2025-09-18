use super::prelude::*;
use anyhow::{Context, Result};
use async_lsp::lsp_types;
use contract::RoutineSpecification;
use contract::{Block, Postcondition, Precondition};
use std::borrow::Borrow;
use std::fmt::Display;
use std::ops::Deref;
use std::path::Path;

mod notes;
pub use notes::Notes;

mod eiffel_type;
pub use eiffel_type::EiffelType;

mod parameters;
pub use parameters::Parameters;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum FeatureVisibility {
    Private,
    Some(ClassID),
    Public,
}

#[derive(Debug, Eq, Clone, Hash)]
pub struct FeatureName(String);

impl FeatureName {
    pub fn new<T: ToString>(name: T) -> Self {
        FeatureName(name.to_string())
    }
}

impl From<String> for FeatureName {
    fn from(value: String) -> Self {
        FeatureName(value)
    }
}

impl Borrow<str> for &FeatureName {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for FeatureName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl Deref for FeatureName {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T> PartialEq<T> for FeatureName
where
    T: AsRef<str> + ?Sized,
{
    fn eq(&self, other: &T) -> bool {
        self.0 == other.as_ref()
    }
}

impl Display for FeatureName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Feature {
    name: FeatureName,
    parameters: Parameters,
    return_type: Option<EiffelType>,
    notes: Option<Notes>,
    visibility: FeatureVisibility,
    range: Range,
    body_range: Option<Range>,
    /// Is None only when a precondition cannot be added (for attributes without an attribute clause).
    preconditions: Option<Block<Precondition>>,
    postconditions: Option<Block<Postcondition>>,
}

impl Feature {
    pub fn new<N>(
        name: N,
        parameters: Parameters,
        return_type: Option<EiffelType>,
        notes: Option<Notes>,
        visibility: FeatureVisibility,
        range: Range,
        body_range: Option<Range>,
        preconditions: Option<Block<Precondition>>,
        postconditions: Option<Block<Postcondition>>,
    ) -> Self
    where
        N: ToString,
    {
        Self {
            name: FeatureName(name.to_string()),
            parameters,
            return_type,
            notes,
            visibility,
            range,
            body_range,
            preconditions,
            postconditions,
        }
    }

    pub fn move_one_line_up(&mut self) {
        let Feature {
            name: _,
            parameters: _,
            return_type: _,
            notes: _,
            visibility: _,
            range,
            body_range,
            preconditions,
            postconditions,
        } = self;

        if let Some(Block { item: _, range }) = preconditions {
            range.move_one_line_up();
        }

        if let Some(Block { item: _, range }) = postconditions {
            range.move_one_line_up();
        }

        range.move_one_line_up();

        if let Some(range) = body_range {
            range.move_one_line_up();
        }
    }

    pub fn is_feature_around_point(&self, point: Point) -> bool {
        point >= self.range().start && point <= self.range().end
    }

    pub fn feature_around_point<'feature>(
        mut features: impl Iterator<Item = &'feature Feature>,
        point: Point,
    ) -> Option<&'feature Feature> {
        features.find(|f| f.is_feature_around_point(point))
    }

    pub fn clone_rename<T: ToString>(&self, name: T) -> Feature {
        let mut f = self.clone();
        f.name = FeatureName(name.to_string());
        f
    }

    pub fn name(&self) -> &FeatureName {
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

    pub fn body_range(&self) -> Option<&Range> {
        self.body_range.as_ref()
    }

    pub fn notes(&self) -> Option<&Notes> {
        self.notes.as_ref()
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
        self.preconditions.as_ref().map(|pre| pre.range().end)
    }

    pub fn point_start_preconditions(&self) -> Option<Point> {
        self.preconditions.as_ref().map(|post| post.range().start)
    }

    pub fn point_end_postconditions(&self) -> Option<Point> {
        self.postconditions.as_ref().map(|post| post.range().end)
    }

    pub fn point_start_postconditions(&self) -> Option<Point> {
        self.postconditions.as_ref().map(|post| post.range().start)
    }

    pub fn supports_precondition_block(&self) -> bool {
        self.preconditions.is_some()
    }

    pub fn supports_postcondition_block(&self) -> bool {
        self.postconditions.is_some()
    }

    fn source_in_range_unchecked<T: Borrow<str>>(&self, source: T, range: Range) -> Result<String> {
        let Range {
            start:
                Point {
                    row: start_row,
                    column: start_column,
                },
            end:
                Point {
                    row: end_row,
                    column: end_column,
                },
        } = range;

        let feature = source
            .borrow()
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

    async fn external_source_in_range_unchecked(
        &self,
        path: &Path,
        range: Range,
    ) -> Result<String> {
        Self::source_in_range_unchecked(
            self,
            String::from_utf8(tokio::fs::read(&path).await?)?,
            range,
        )
    }

    pub async fn source_unchecked(&self, path: &Path) -> Result<String> {
        self.external_source_in_range_unchecked(path, self.range().clone())
            .await
    }

    pub async fn body_source_unchecked_at_path(&self, path: &Path) -> Result<String> {
        let mut body = self
            .body_range()
            .with_context(|| format!("fails to get body range of feature: {}", self.name()))?
            .clone();

        // Ignore the do keyword in the range.
        body.start.column += 2;

        self.external_source_in_range_unchecked(path, body).await
    }

    pub fn body_source_unchecked<T: Borrow<str>>(&self, source: T) -> Result<String> {
        let mut body = self
            .body_range()
            .with_context(|| format!("fails to get body range of feature: {}", self.name()))?
            .clone();

        // Ignore the do keyword in the range.
        body.start.column += 2;

        self.source_in_range_unchecked(source, body)
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
        let format_return_type = self
            .return_type()
            .map_or_else(String::new, |ref return_type| format!(": {return_type}"));
        write!(f, "{name} {parenthesized_parameters}{format_return_type}")
    }
}

impl TryFrom<&Feature> for lsp_types::DocumentSymbol {
    type Error = anyhow::Error;

    #[allow(deprecated)]
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
    use super::*;
    use crate::parser::Parsed;
    use crate::parser::Parser;

    fn class(src: &str) -> Result<Class> {
        let mut parser = Parser::default();
        parser.class_from_source(src)
    }

    #[test]
    fn parse_feature_with_precondition() -> Result<()> {
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
        assert!(
            eiffeltype
                .class_name()
                .is_ok_and(|name| name == *"MML_SEQUENCE")
        );
    }

    #[test]
    fn body_src() {
        let src = r#"min (x, y: INTEGER): INTEGER
		do
		    if x < y then
    		    Result := x
    		else
    		    y := y
    		end
		end"#;
        let mut parser = Parser::default();

        match parser.to_feature(src).expect("Should parse `min` feature.") {
            Parsed::Correct(feature) => {
                let body_src = feature.body_source_unchecked(src).unwrap_or_else(|e| {
                    panic!(
                        "Should extract body from the feature {:#?} with the code {}\nbecause {:#?}",
                        feature, src, e
                    )
                });
                assert_eq!(
                    body_src,
                    r#"
		    if x < y then
    		    Result := x
    		else
    		    y := y
    		end
"#
                )
            }
            Parsed::HasErrorNodes(tree, _) => {
                unreachable!(
                    "The parsing of `min` should be correct. Instead, it returns the following tree: {}",
                    tree.root_node().to_sexp()
                )
            }
        }
    }
}
