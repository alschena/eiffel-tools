use super::prelude::*;
use crate::lib::code_entities::feature::Notes;
use crate::lib::tree_sitter_extension::Parse;
use anyhow::anyhow;
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
    fn valid(&self, system_classes: &[&Class], current_class: &Class) -> bool {
        self.decorated_valid_syntax()
            && self.decorated_valid_identifiers(system_classes, current_class)
            && self.decorated_valid_calls(system_classes, current_class)
    }
    fn valid_syntax(&self) -> bool;
    fn valid_top_level_identifiers(&self, system_classes: &[&Class], current_class: &Class)
        -> bool;
    fn valid_top_level_calls(&self, _system_classes: &[&Class], _current_class: &Class) -> bool {
        true
    }

    fn decorated_valid_syntax(&self) -> bool {
        let value = self.valid_syntax();
        if !value {
            info!(target: "gemini","filtered by syntax {self:?}");
        }
        value
    }
    fn decorated_valid_identifiers(
        &self,
        system_classes: &[&Class],
        current_class: &Class,
    ) -> bool {
        let value = self.valid_top_level_identifiers(system_classes, current_class);
        if !value {
            info!(target: "gemini","filtered by invalid identifier {self:?}");
        }
        value
    }
    fn decorated_valid_calls(&self, system_classes: &[&Class], current_class: &Class) -> bool {
        let value = self.valid_top_level_calls(system_classes, current_class);
        if !value {
            info!(target: "gemini","filtered by invalid top level call {self:?}");
        }
        value
    }
}
pub trait Type {
    fn query() -> Query;
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
    ) -> bool {
        self.predicate
            .valid_top_level_identifiers(system_classes, current_class)
            && self
                .tag
                .valid_top_level_identifiers(system_classes, current_class)
    }
}
impl Parse for Clause {
    type Error = anyhow::Error;
    fn parse(assertion_clause: &Node, src: &str) -> anyhow::Result<Self> {
        debug_assert_eq!(assertion_clause.kind(), "assertion_clause");
        debug_assert!(assertion_clause.child_count() > 0);

        let lang = &tree_sitter_eiffel::LANGUAGE.into();
        let clause_query =
            ::tree_sitter::Query::new(lang, "((tag_mark (tag) @tag)? (expression) @expr)").unwrap();

        let mut binding = QueryCursor::new();
        let mut captures =
            binding.captures(&clause_query, assertion_clause.clone(), src.as_bytes());

        match captures.next() {
            Some(&(ref m, _)) => {
                let tag_node = m.nodes_for_capture_index(0).next();
                let expression_node = m.nodes_for_capture_index(1).next().unwrap();

                let tag: Tag = match tag_node {
                    Some(n) => src[n.byte_range()].to_string().into(),
                    None => String::new().into(),
                };

                let predicate = Predicate::new(src[expression_node.byte_range()].to_string());

                Ok(Self { predicate, tag })
            }
            None => Err(anyhow!("Wrong arguments, should match")),
        }
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
    ) -> bool {
        let ids = self.top_level_identifiers();
        ids.iter().all(|&identifier| {
            current_class
                .features()
                .iter()
                .map(|feature| std::borrow::Cow::Borrowed(feature))
                .chain(current_class.inhereted_features(system_classes))
                .any(|feature| feature.name() == identifier)
        })
    }
    /// NOTE: For now only checks the number of arguments of each call is correct.
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
    ) -> bool {
        self.iter()
            .all(|clause| clause.valid_top_level_identifiers(system_classes, current_class))
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
    fn query() -> Query {
        Query::new(&tree_sitter_eiffel::LANGUAGE.into(), "(precondition) @x")
            .expect("fails to create precondition query.")
    }
    fn keyword() -> Keyword {
        Keyword::Require
    }
}
impl Parse for Block<Precondition> {
    type Error = anyhow::Error;

    fn parse(node: &Node, src: &str) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "attribute_or_routine");

        let mut cursor = QueryCursor::new();
        let lang = &tree_sitter_eiffel::LANGUAGE.into();
        let query = Precondition::query();
        let mut contracts_captures = cursor.captures(&query, node.clone(), src.as_bytes());
        let contracts_cap = contracts_captures.next();
        let node = match contracts_cap {
            Some(x) => x.0.captures[0].node,
            None => {
                let notes_query = Notes::query();
                let point = match cursor
                    .matches(&notes_query, node.clone(), src.as_bytes())
                    .next()
                {
                    Some(notes) => Point::from(notes.captures[0].node.range().start_point),
                    None => Point::from(node.range().start_point),
                };
                return Ok(Self::new_empty(point));
            }
        };

        let query = Query::new(lang, "(assertion_clause (expression)) @x").unwrap();
        let mut assertion_clause_matches = cursor.matches(&query, node.clone(), src.as_bytes());

        let mut clauses: Vec<Clause> = Vec::new();
        while let Some(mat) = assertion_clause_matches.next() {
            for cap in mat.captures {
                clauses.push(Clause::parse(&cap.node, src)?)
            }
        }

        Ok(Self::new(clauses.into(), node.range().into()))
    }
}
impl Parse for Block<Postcondition> {
    type Error = anyhow::Error;

    fn parse(node: &Node, src: &str) -> Result<Self, Self::Error> {
        debug_assert!(node.kind() == "attribute_or_routine");

        let mut cursor = QueryCursor::new();
        let lang = &tree_sitter_eiffel::LANGUAGE.into();
        let query = Postcondition::query();
        let mut contracts_captures = cursor.captures(&query, node.clone(), src.as_bytes());
        let contracts_cap = contracts_captures.next();
        let node = match contracts_cap {
            Some(x) => x.0.captures[0].node,
            None => {
                let mut point = Point::from(node.range().end_point);
                // This compensates the keyword `end`.
                point.shift_left(3);
                return Ok(Self::new_empty(point));
            }
        };

        let query = Query::new(lang, "(assertion_clause (expression)) @x").unwrap();
        let mut assertion_clause_matches = cursor.matches(&query, node.clone(), src.as_bytes());

        let mut clauses: Vec<Clause> = Vec::new();
        while let Some(mat) = assertion_clause_matches.next() {
            for cap in mat.captures {
                clauses.push(Clause::parse(&cap.node, src)?)
            }
        }

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
    ) -> bool {
        self.iter()
            .all(|clause| clause.valid_top_level_identifiers(system_classes, current_class))
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
    ) -> bool {
        self.precondition
            .valid_top_level_identifiers(system_classes, current_class)
            && self
                .postcondition
                .valid_top_level_identifiers(system_classes, current_class)
    }
}
impl From<Vec<Clause>> for Postcondition {
    fn from(value: Vec<Clause>) -> Self {
        Self(value)
    }
}
impl Type for Postcondition {
    fn query() -> Query {
        Query::new(&tree_sitter_eiffel::LANGUAGE.into(), "(postcondition) @x")
            .expect("fails to create postcondition query.")
    }
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
    use crate::lib::code_entities::class::Ancestor;

    use super::*;
    use anyhow::Result;
    use assert_fs::prelude::*;
    use assert_fs::{fixture::FileWriteStr, TempDir};
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
        let clause = Clause::parse(&node, &src).expect("Parse feature");
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
        let mut parser = ::tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();

        let query = ::tree_sitter::Query::new(
            &tree_sitter_eiffel::LANGUAGE.into(),
            "(attribute_or_routine) @x",
        )
        .unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());

        let node = captures.next().unwrap().0.captures[0].node;

        let mut precondition =
            <Block<Precondition>>::parse(&node, &src).expect("fails to parse precondition.");
        let predicate = Predicate::new("True".to_string());
        let tag = Tag(String::new());
        let clause = precondition.item.pop().expect("Parse clause");
        assert_eq!(clause.predicate, predicate);
        assert_eq!(clause.tag, tag);
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
        let mut parser = ::tree_sitter::Parser::new();
        parser
            .set_language(&tree_sitter_eiffel::LANGUAGE.into())
            .expect("Error loading Eiffel grammar");
        let tree = parser.parse(src, None).unwrap();

        let query = ::tree_sitter::Query::new(
            &tree_sitter_eiffel::LANGUAGE.into(),
            "(attribute_or_routine) @x",
        )
        .unwrap();

        let mut binding = QueryCursor::new();
        let mut captures = binding.captures(&query, tree.root_node(), src.as_bytes());

        let node = captures.next().unwrap().0.captures[0].node;

        let mut postcondition =
            <Block<Postcondition>>::parse(&node, &src).expect("fails to parse postcondition.");
        let predicate = Predicate::new("True".to_string());
        let tag = Tag(String::new());
        let clause = postcondition.item.pop().expect("Parse clause");
        assert_eq!(clause.predicate, predicate);
        assert_eq!(clause.tag, tag);
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
            end
        ";
        let class = Class::from_source(src);
        let system_classes = vec![&class];

        // Create an invalid and a valid predicates.
        let invalid_predicate = Predicate(String::from("z"));
        let valid_predicate = Predicate(String::from("x"));

        assert!(!invalid_predicate.valid(&system_classes, &class));
        assert!(valid_predicate.valid(&system_classes, &class));
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
            end
        ";

        let parent = Class::from_source(parent_src);
        let child = Class::from_source(child_src);

        assert!(child
            .features()
            .into_iter()
            .find(|f| f.name() == "x")
            .is_none());

        let system_classes = vec![&child, &parent];
        let valid_predicate = Predicate(String::from("x"));
        assert!(valid_predicate.valid(&system_classes, &child));
    }
}
