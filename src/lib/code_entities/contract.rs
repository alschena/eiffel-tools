use super::prelude::*;
use crate::lib::tree_sitter_extension::capture_name_to_nodes;
use crate::lib::tree_sitter_extension::node_to_text;
use crate::lib::tree_sitter_extension::Parse;
use gemini::{Described, ResponseSchema, ToResponseSchema};
use gemini_macro_derive::ToResponseSchema;
use serde::Deserialize;
use std::collections::HashSet;
use std::fmt::Debug;
use std::fmt::Display;
use std::ops::Deref;
use std::ops::DerefMut;
use streaming_iterator::StreamingIterator;
use tracing::info;
use tree_sitter::{Node, Query, QueryCursor, Tree};
pub(crate) trait Valid: Debug {
    fn valid(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.decorated_valid_syntax()
            && self.decorated_valid_top_level_identifiers(
                system_classes,
                current_class,
                current_feature,
            )
            && self.decorated_valid_calls(system_classes, current_class)
            && self.decorated_valid_no_repetition(system_classes, current_class, current_feature)
    }
    fn valid_syntax(&self) -> bool;
    fn decorated_valid_syntax(&self) -> bool {
        let value = self.valid_syntax();
        if !value {
            info!(target: "gemini","filtered by syntax {self:?}");
        }
        value
    }
    fn valid_top_level_identifiers(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool;
    fn decorated_valid_top_level_identifiers(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        let value =
            self.valid_top_level_identifiers(system_classes, current_class, current_feature);
        if !value {
            info!(target: "gemini","filtered by invalid identifier {self:?}");
        }
        value
    }
    fn valid_top_level_calls(&self, _system_classes: &[&Class], _current_class: &Class) -> bool {
        true
    }
    fn decorated_valid_calls(&self, system_classes: &[&Class], current_class: &Class) -> bool {
        let value = self.valid_top_level_calls(system_classes, current_class);
        if !value {
            info!(target: "gemini","filtered by invalid top level call {self:?}");
        }
        value
    }
    fn valid_no_repetition(
        &self,
        _system_classes: &[&Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> bool {
        true
    }
    fn decorated_valid_no_repetition(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        let value = self.valid_no_repetition(system_classes, current_class, current_feature);
        if !value {
            info!(target: "gemini","filter because the clause is repeated.");
        }
        value
    }
}
pub trait Type {
    fn keyword() -> Keyword;
}
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
/// Wraps an optional contract clause adding whereabouts informations.
/// If the `item` is None, the range start and end coincide where the contract clause would be added.
pub struct Block<T> {
    pub item: T,
    pub range: Range,
}
impl<T: Type> Block<T> {
    pub fn item(&self) -> &T {
        &self.item
    }
    pub fn range(&self) -> &Range {
        &self.range
    }
    pub fn new(item: T, range: Range) -> Self {
        Self { item, range }
    }
}
impl<T: Type + Default> Block<T> {
    pub fn new_empty(point: Point) -> Self {
        Self {
            item: T::default(),
            range: Range::new_collapsed(point),
        }
    }
}
impl<T: Indent> Indent for Block<T> {
    const INDENTATION_LEVEL: usize = T::INDENTATION_LEVEL - 1;
}
impl<T: Display + Indent + Type + Deref<Target = Vec<Clause>>> Display for Block<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.item().is_empty() {
            write!(f, "")
        } else {
            write!(
                f,
                "{}{}\n{}",
                T::keyword(),
                &self.item,
                Self::indentation_string(),
            )
        }
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum Keyword {
    Require,
    RequireThen,
    Ensure,
    EnsureElse,
    Invariant,
}
impl Display for Keyword {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let content = match &self {
            Keyword::Require => "require",
            Keyword::RequireThen => "require then",
            Keyword::Ensure => "ensure",
            Keyword::EnsureElse => "ensure else",
            Keyword::Invariant => "invariant",
        };
        write!(f, "{}", content)
    }
}
#[derive(Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone, Hash)]
pub struct Clause {
    pub tag: Tag,
    pub predicate: Predicate,
}
impl Default for Clause {
    fn default() -> Self {
        Self {
            tag: <Tag as Default>::default(),
            predicate: <Predicate as Default>::default(),
        }
    }
}
impl Valid for Clause {
    fn valid_syntax(&self) -> bool {
        self.predicate.valid_syntax() && self.tag.valid_syntax()
    }
    fn valid_top_level_identifiers(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.predicate
            .valid_top_level_identifiers(system_classes, current_class, current_feature)
            && self
                .tag
                .valid_top_level_identifiers(system_classes, current_class, current_feature)
    }
    fn valid_top_level_calls(&self, system_classes: &[&Class], current_class: &Class) -> bool {
        self.tag
            .valid_top_level_calls(system_classes, current_class)
            && self
                .predicate
                .valid_top_level_calls(system_classes, current_class)
    }
}
impl Parse for Clause {
    type Error = anyhow::Error;
    fn parse(assertion_clause: &Node, cursor: &mut QueryCursor, src: &str) -> anyhow::Result<Self> {
        debug_assert_eq!(assertion_clause.kind(), "assertion_clause");
        debug_assert!(assertion_clause.child_count() > 0);

        let clause_query = Self::query("((tag_mark (tag) @tag)? (expression) @expr)");

        let mut matches = cursor.matches(&clause_query, assertion_clause.clone(), src.as_bytes());
        let mat = matches.next().expect("match a clause.");

        let tag: Tag = capture_name_to_nodes("tag", &clause_query, mat)
            .next()
            .map_or_else(
                || Tag(String::new()),
                |tag| Tag(node_to_text(&tag, src).to_string()),
            );

        let predicate: Predicate = capture_name_to_nodes("expr", &clause_query, mat)
            .next()
            .map(|predicate| Predicate::new(node_to_text(&predicate, src).to_string()))
            .expect("clauses have predicates.");
        Ok(Self { predicate, tag })
    }
}
impl Display for Clause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}\n", self.tag, self.predicate)
    }
}
impl Clause {
    pub fn new(tag: Tag, predicate: Predicate) -> Clause {
        Clause { tag, predicate }
    }
}
#[derive(Deserialize, Clone, ToResponseSchema, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct Tag(String);

impl Tag {
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Tag {
    fn default() -> Self {
        Self(String::from("default"))
    }
}

impl Valid for Tag {
    fn valid_syntax(&self) -> bool {
        !self.as_str().contains(" ")
    }
    fn valid_top_level_identifiers(
        &self,
        _system_classes: &[&Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> bool {
        true
    }
}
impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
impl From<String> for Tag {
    fn from(value: String) -> Self {
        Tag(value)
    }
}
#[derive(Hash, Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone)]
#[serde(transparent)]
pub struct Predicate(String);

impl Predicate {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn parse(&self) -> Option<Tree> {
        let text: &str = self.as_str();
        let lang = tree_sitter_eiffel::LANGUAGE.into();
        let mut parser = tree_sitter::Parser::new();
        parser
            .set_language(&lang)
            .expect("parser must load grammar.");
        parser.parse(text, None)
    }

    fn top_level_identifiers(&self) -> HashSet<&str> {
        let tree = self.parse().expect("fails to parse predicate.");
        let lang = tree_sitter_eiffel::LANGUAGE.into();
        let text = self.as_str();

        let query_id = Query::new(&lang, "(call (unqualified_call (identifier) @id) !target)")
            .expect("Fails to construct query for top-level identifiers (names of unqualified features and targets) in predicate: {self}");

        let mut query_cursor = QueryCursor::new();

        let mut matches = query_cursor.matches(&query_id, tree.root_node(), text.as_bytes());

        let mut ids = HashSet::new();
        while let Some(mat) = matches.next() {
            for cap in mat.captures.iter() {
                let id = cap
                    .node
                    .utf8_text(text.as_bytes())
                    .expect("The capture must contain valid text.");
                ids.insert(id);
            }
        }
        ids
    }

    fn top_level_calls_with_arguments(&self) -> Vec<(&str, Vec<&str>)> {
        let tree = self.parse().expect("fails to parse predicate.");
        let lang = tree_sitter_eiffel::LANGUAGE.into();
        let text = self.as_str();

        let query_id = Query::new(
            &lang,
            r#"(call (unqualified_call (identifier) @id
            (actuals (expression) @argument
                ("," (expression) @argument)*) !target))"#,
        )
        .expect("Fails to construct query for top-level calls with arguments in predicate: {self}");

        let mut query_cursor = QueryCursor::new();

        let mut matches = query_cursor.matches(&query_id, tree.root_node(), text.as_bytes());

        let mut calls_with_args = Vec::new();
        while let Some(mat) = matches.next() {
            let mut args = Vec::new();
            let name: &str;

            mat.nodes_for_capture_index(
                query_id
                    .capture_index_for_name("argument")
                    .expect("`argument` is a capture name."),
            )
            .for_each(|node| {
                args.push(
                    node.utf8_text(text.as_bytes())
                        .expect("valid capture for call's argument."),
                )
            });

            let id_node = mat
                .nodes_for_capture_index(
                    query_id
                        .capture_index_for_name("id")
                        .expect("`id` is a capture name."),
                )
                .next()
                .expect("Calls must have an identifier.");
            name = id_node
                .utf8_text(text.as_bytes())
                .expect("valid capture for call's identifier.");

            calls_with_args.push((name, args));
        }
        calls_with_args
    }
}

impl Default for Predicate {
    fn default() -> Self {
        Self(String::from("True"))
    }
}

impl Valid for Predicate {
    fn valid_syntax(&self) -> bool {
        match self.parse() {
            Some(tree) => !tree.root_node().has_error(),
            None => {
                info!("fails to parse predicate: {}", self.as_str());
                false
            }
        }
    }
    fn valid_top_level_identifiers(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        let ids = self.top_level_identifiers();
        ids.iter().all(|&identifier| {
            current_class
                .features()
                .iter()
                .map(|feature| std::borrow::Cow::Borrowed(feature))
                .chain(current_class.inhereted_features(system_classes))
                .any(|feature| {
                    current_feature
                        .parameters()
                        .iter()
                        .any(|(name, _)| name == identifier)
                        || (identifier == feature.name())
                })
        })
    }
    /// NOTE: For now only checks the number of arguments of each unqualified call is correct.
    fn valid_top_level_calls(&self, system_classes: &[&Class], current_class: &Class) -> bool {
        let calls = self.top_level_calls_with_arguments();
        calls.iter().all(|&(id, ref args)| {
            current_class
                .features()
                .iter()
                .map(|feature| std::borrow::Cow::Borrowed(feature))
                .chain(current_class.inhereted_features(system_classes))
                .find(|feature| feature.name() == id)
                .is_some_and(|feature| feature.number_parameters() == args.len())
        })
    }
}
impl Display for Predicate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
impl Predicate {
    fn new(s: String) -> Predicate {
        Predicate(s)
    }
}
#[derive(Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone, Hash)]
#[serde(transparent)]
pub struct Precondition(Vec<Clause>);

impl Deref for Precondition {
    type Target = Vec<Clause>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl DerefMut for Precondition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Default for Precondition {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl Valid for Precondition {
    fn valid_syntax(&self) -> bool {
        self.iter().all(|clause| clause.valid_syntax())
    }
    fn valid_top_level_identifiers(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.iter().all(|clause| {
            clause.valid_top_level_identifiers(system_classes, current_class, current_feature)
        })
    }
    fn valid_top_level_calls(&self, system_classes: &[&Class], current_class: &Class) -> bool {
        self.iter()
            .all(|clause| clause.valid_top_level_calls(system_classes, current_class))
    }
    fn valid_no_repetition(
        &self,
        _system_classes: &[&Class],
        _current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        current_feature.preconditions().is_none_or(|pre| {
            self.iter()
                .map(|clause| &clause.predicate)
                .all(|predicate| pre.iter().any(|c| &c.predicate == predicate))
        })
    }
}
impl From<Vec<Clause>> for Precondition {
    fn from(value: Vec<Clause>) -> Self {
        Self(value)
    }
}
impl Indent for Precondition {
    const INDENTATION_LEVEL: usize = 3;
}
impl Type for Precondition {
    fn keyword() -> Keyword {
        Keyword::Require
    }
}
impl Parse for Block<Precondition> {
    type Error = anyhow::Error;

    fn parse(node: &Node, cursor: &mut QueryCursor, src: &str) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "precondition");
        let query = Self::query("(assertion_clause (expression))* @assertion_clause");

        let clauses: Vec<_> = cursor
            .matches(&query, node.clone(), src.as_bytes())
            .map_deref(|mat| mat.captures)
            .flatten()
            .filter_map(|cap| Clause::parse(&cap.node, &mut QueryCursor::new(), src).ok())
            .collect();

        Ok(Self::new(clauses.into(), node.range().into()))
    }
}
impl Parse for Block<Postcondition> {
    type Error = anyhow::Error;

    fn parse(node: &Node, cursor: &mut QueryCursor, src: &str) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "postcondition");
        let query = Self::query("(assertion_clause (expression))* @assertion_clause");

        let clauses: Vec<_> = cursor
            .matches(&query, node.clone(), src.as_bytes())
            .map_deref(|mat| mat.captures)
            .flatten()
            .filter_map(|cap| Clause::parse(&cap.node, &mut QueryCursor::new(), src).ok())
            .collect();

        Ok(Self::new(clauses.into(), node.range().into()))
    }
}

#[derive(Hash, Deserialize, ToResponseSchema, Debug, PartialEq, Eq, Clone)]
#[serde(transparent)]
pub struct Postcondition(Vec<Clause>);

impl Deref for Postcondition {
    type Target = Vec<Clause>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Postcondition {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

impl Default for Postcondition {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl Valid for Postcondition {
    fn valid_syntax(&self) -> bool {
        self.iter().all(|clause| clause.valid_syntax())
    }
    fn valid_top_level_identifiers(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.iter().all(|clause| {
            clause.valid_top_level_identifiers(system_classes, current_class, current_feature)
        })
    }
    fn valid_top_level_calls(&self, system_classes: &[&Class], current_class: &Class) -> bool {
        self.iter()
            .all(|clause| clause.valid_top_level_calls(system_classes, current_class))
    }
    fn valid_no_repetition(
        &self,
        _system_classes: &[&Class],
        _current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        current_feature.postconditions().is_none_or(|post| {
            self.iter()
                .map(|clause| &clause.predicate)
                .all(|postdicate| post.iter().any(|c| &c.predicate == postdicate))
        })
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize, ToResponseSchema)]
pub struct RoutineSpecification {
    pub precondition: Precondition,
    pub postcondition: Postcondition,
}
impl Valid for RoutineSpecification {
    fn valid_syntax(&self) -> bool {
        self.precondition.valid_syntax() && self.postcondition.valid_syntax()
    }
    fn valid_top_level_identifiers(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.precondition.valid_top_level_identifiers(
            system_classes,
            current_class,
            current_feature,
        ) && self.postcondition.valid_top_level_identifiers(
            system_classes,
            current_class,
            current_feature,
        )
    }
    fn valid_top_level_calls(&self, system_classes: &[&Class], current_class: &Class) -> bool {
        self.precondition
            .valid_top_level_calls(system_classes, current_class)
            && self
                .postcondition
                .valid_top_level_calls(system_classes, current_class)
    }
    fn valid_no_repetition(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.precondition
            .valid_no_repetition(system_classes, current_class, current_feature)
            && self.postcondition.valid_no_repetition(
                system_classes,
                current_class,
                current_feature,
            )
    }
}
impl From<Vec<Clause>> for Postcondition {
    fn from(value: Vec<Clause>) -> Self {
        Self(value)
    }
}
impl Type for Postcondition {
    fn keyword() -> Keyword {
        Keyword::Ensure
    }
}
impl Indent for Postcondition {
    const INDENTATION_LEVEL: usize = 3;
}
impl Display for Precondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.iter()
                .fold(String::from('\n'), |mut acc, elt| {
                    acc.push_str(format!("{}{}", Self::indentation_string(), elt).as_str());
                    acc
                })
                .trim_end()
        )
    }
}
impl Display for Postcondition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            self.iter()
                .fold(String::from('\n'), |mut acc, elt| {
                    acc.push_str(format!("{}{}", Self::indentation_string(), elt).as_str());
                    acc
                })
                .trim_end()
        )
    }
}
impl Described for Clause {
    fn description() -> String {
        String::from("A valid contract clause of the eiffel programming language.")
    }
}
impl Described for Tag {
    fn description() -> String {
        "A valid tag clause for the Eiffel programming language.".to_string()
    }
}
impl Described for Predicate {
    fn description() -> String {
        "A valid boolean expression for the Eiffel programming language.".to_string()
    }
}
impl Described for Precondition {
    fn description() -> String {
        "Preconditions are predicates on the prestate, the state before the execution, of a routine. They describe the properties that the fields of the model in the current object must satisfy in the prestate. Preconditions cannot contain a call to `old_` or the `old` keyword.".to_string()
    }
}
impl Described for Postcondition {
    fn description() -> String {
        "Postconditions describe the properties that the model of the current object must satisfy after the routine.
        Postconditions are two-states predicates.
        They can refer to the prestate of the routine by calling the feature `old_` on any object which existed before the execution of the routine.
        Equivalently, you can use the keyword `old` before a feature to access its prestate.".to_string()
    }
}
impl Described for RoutineSpecification {
    fn description() -> String {
        String::new()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    use gemini::SchemaType;
    use Clause;

    #[test]
    fn parse_clause() {
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
        let query = ::tree_sitter::Query::new(
            &tree_sitter_eiffel::LANGUAGE.into(),
            "(assertion_clause) @x",
        )
        .unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());

        let node = captures.next().unwrap().0.captures[0].node;
        let clause = Clause::parse(&node, &mut binding, &src).expect("Parse feature");
        assert_eq!(clause.tag, Tag::from(String::new()));
        assert_eq!(clause.predicate, Predicate::new("True".to_string()));
    }
    #[test]
    fn parse_precondition() {
        let src = r#"
class A feature
  x
    require
      True
    do
    end
end"#;
        let class = Class::from_source(src);
        let feature = class
            .features()
            .first()
            .expect("class `A` has feature `x`.");
        eprintln!("{feature:?}");

        assert!(feature.supports_precondition_block());
        assert!(feature.supports_postcondition_block());

        let precondition = feature
            .preconditions()
            .expect("feature has precondition block.");

        let clause = precondition
            .first()
            .expect("precondition block has trivial assertion clause.");

        let predicate = &clause.predicate;
        let tag = &clause.tag;

        assert_eq!(*predicate, Predicate::new("True".to_string()));
        assert_eq!(*tag, Tag(String::new()));
    }
    #[test]
    fn parse_postcondition() {
        let src = r#"
class A feature
  x
    do
    ensure then
      True
    end
end"#;
        let class = Class::from_source(src);
        let feature = class.features().first().expect("first feature is `x`.");

        let postcondition = feature
            .postconditions()
            .expect("postcondition block with trivial postcondition.");

        let clause = postcondition
            .first()
            .expect("trivial postcondition clause.");
        assert_eq!(postcondition.len(), 1);

        assert_eq!(&clause.predicate, &Predicate::new("True".to_string()));
        assert_eq!(&clause.tag, &Tag(String::new()));
    }
    // For gemini completions.
    // When the LSP grows in maturity, gemini will be decoupled and these tests will be moved to a compatibility layer.
    #[test]
    fn precondition_response_schema() -> Result<()> {
        let response_schema = Precondition::to_response_schema();
        let oracle_response = ResponseSchema {
            schema_type: SchemaType::Array,
            format: None,
            description: Some(Precondition::description()),
            nullable: None,
            possibilities: None,
            max_items: None,
            properties: None,
            required: None,
            items: Some(Box::new(Clause::to_response_schema())),
        };
        assert_eq!(response_schema, oracle_response);
        Ok(())
    }
    #[test]
    fn postcondition_response_schema() -> Result<()> {
        let response_schema = Postcondition::to_response_schema();
        let oracle_response = ResponseSchema {
            schema_type: SchemaType::Array,
            format: None,
            description: Some(Postcondition::description()),
            nullable: None,
            possibilities: None,
            max_items: None,
            properties: None,
            required: None,
            items: Some(Box::new(Clause::to_response_schema())),
        };
        assert_eq!(response_schema, oracle_response);
        Ok(())
    }
    #[test]
    fn clause_response_schema() -> Result<()> {
        let response_schema = Clause::to_response_schema();
        let oracle_schema_type = SchemaType::Object;
        let oracle_format = None;
        let oracle_description = Some(Clause::description());
        let oracle_nullable = None;
        let oracle_possibilities = None;
        let oracle_max_items = None;
        let oracle_properties = Some(std::collections::HashMap::from([
            (String::from("tag"), Tag::to_response_schema()),
            (String::from("predicate"), Predicate::to_response_schema()),
        ]));
        let oracle_required = Some(vec![String::from("tag"), String::from("predicate")]);
        let oracle_items = None;
        assert_eq!(response_schema.schema_type, oracle_schema_type);
        assert_eq!(response_schema.format, oracle_format);
        assert_eq!(response_schema.description, oracle_description);
        assert_eq!(response_schema.nullable, oracle_nullable);
        assert_eq!(response_schema.possibilities, oracle_possibilities);
        assert_eq!(response_schema.max_items, oracle_max_items);
        assert_eq!(response_schema.properties, oracle_properties);
        assert_eq!(
            response_schema.required.map(|r| r
                .clone()
                .into_iter()
                .collect::<std::collections::HashSet<_>>()),
            oracle_required.map(|r| { r.clone().into_iter().collect() })
        );
        assert_eq!(response_schema.items, oracle_items);
        Ok(())
    }
    #[test]
    fn tag_response_schema() -> Result<()> {
        let response_schema = Tag::to_response_schema();
        let oracle_response = ResponseSchema {
            schema_type: SchemaType::String,
            format: None,
            description: Some(Tag::description()),
            nullable: None,
            possibilities: None,
            max_items: None,
            properties: None,
            required: None,
            items: None,
        };
        assert_eq!(response_schema, oracle_response);
        Ok(())
    }
    #[test]
    fn predicate_response_schema() -> Result<()> {
        let response_schema = Predicate::to_response_schema();
        let oracle_response = ResponseSchema {
            schema_type: SchemaType::String,
            format: None,
            description: Some(Predicate::description()),
            nullable: None,
            possibilities: None,
            max_items: None,
            properties: None,
            required: None,
            items: None,
        };
        assert_eq!(response_schema, oracle_response);
        Ok(())
    }
    #[test]
    fn predicate_valid_syntax() {
        let invalid_predicate = Predicate::new("min min".into());
        let valid_predicate = Predicate::new("min (x, y)".into());
        assert!(!invalid_predicate.valid_syntax());
        assert!(valid_predicate.valid_syntax());
    }
    #[test]
    fn tag_valid_syntax() {
        let invalid_tag: Tag = String::from("this is not valid").into();
        let valid_tag: Tag = String::from("this_is_valid").into();
        assert!(!invalid_tag.valid_syntax());
        assert!(valid_tag.valid_syntax());
    }
    #[test]
    fn display_precondition_block() {
        let empty_block: Block<Precondition> = Block::new_empty(Point { row: 0, column: 0 });
        let simple_block = Block::new(
            Precondition(vec![Clause {
                tag: Tag::default(),
                predicate: Predicate::default(),
            }]),
            Range::new(Point { row: 0, column: 0 }, Point { row: 0, column: 4 }),
        );
        assert_eq!(format!("{empty_block}"), "");
        assert_eq!(
            format!("{simple_block}"),
            "require\n\t\t\tdefault: True\n\t\t"
        );
    }
    #[test]
    fn predicate_identifiers() {
        let p = Predicate("x < y.z.w".to_string());
        let ids = p.top_level_identifiers();
        assert!(ids.contains("x"));
        assert!(ids.contains("y"));
        assert!(ids.len() == 2);
    }
    #[test]
    fn predicate_identifiers_unqualified_calls() {
        let p = Predicate("x (y) < y (l).z.w".to_string());
        let ids = p.top_level_identifiers();
        eprintln!("{ids:?}");
        assert!(ids.contains("x"));
        assert!(ids.contains("y"));
        assert!(ids.contains("l"));
        assert!(ids.len() == 3);
    }
    #[test]
    fn valid_and_invalid_predicates() {
        let src = "
            class
                A
            feature
                x: BOOLEAN
                y: BOOLEAN
                    do
                        Result := True
                    end
            end
        ";
        let class = Class::from_source(src);
        let feature = class
            .features()
            .iter()
            .find(|f| f.name() == "y".to_string())
            .expect("parse feature y");
        let system_classes = vec![&class];

        // Create an invalid and a valid predicates.
        let invalid_predicate = Predicate(String::from("z"));
        let valid_predicate = Predicate(String::from("x"));

        assert!(!invalid_predicate.valid(&system_classes, &class, feature));
        assert!(valid_predicate.valid(&system_classes, &class, feature));
    }
    #[test]
    fn valid_predicates_in_ancestors() {
        let parent_src = "
            class
                B
            feature
                x: BOOLEAN
            end
        ";
        let child_src = "
            class
                A
            inherit
                B
            feature
                y: BOOLEAN
                    do
                        Result := True
                    end
            end
        ";

        let parent = Class::from_source(parent_src);
        let child = Class::from_source(child_src);
        let feature = child
            .features()
            .iter()
            .find(|f| f.name() == "y")
            .expect("parse feature y");

        assert!(child
            .features()
            .into_iter()
            .find(|f| f.name() == "x")
            .is_none());

        let system_classes = vec![&child, &parent];
        let valid_predicate = Predicate(String::from("x"));
        assert!(valid_predicate.valid(&system_classes, &child, feature));
    }
    #[test]
    fn valid_predicate_of_parameters() {
        let src = "
            class
                A
            feature
                x (f: BOOLEAN): BOOLEAN
                    do
                        Result := f
                    end
            end
        ";
        let c = Class::from_source(src);
        let f = c.features().first().expect("first feature exists.");
        let vp = Predicate::new("f".to_string());
        let ip = Predicate::new("r".to_string());
        let system_classes = vec![&c];
        assert!(vp.valid(&system_classes, &c, f));
        assert!(!ip.valid(&system_classes, &c, f));
    }
    #[test]
    fn invalid_predicate_for_number_of_arguments() {
        let src = "
            class
                A
            feature
                z: BOOLEAN
                x (f: BOOLEAN): BOOLEAN
                    do
                        Result := f
                    end
                y: BOOLEAN
                    do
                        Result := x
                    end
            end
        ";
        let c = Class::from_source(src);
        let f = c
            .features()
            .iter()
            .find(|f| f.name() == "y")
            .expect("first feature exists.");
        let system_classes = vec![&c];

        let vp = Predicate::new("x (z)".to_string());
        let ip = Predicate::new("x (z, z)".to_string());
        let ip2 = Predicate::new("x ()".to_string());

        assert!(vp.valid(&system_classes, &c, f));
        assert!(!ip.valid(&system_classes, &c, f));
        assert!(!ip2.valid(&system_classes, &c, f));
    }
}
