use crate::lib::tree_sitter_extension::capture_name_to_nodes;
use crate::lib::tree_sitter_extension::node_to_text;
use crate::lib::tree_sitter_extension::Parse;
use serde::Deserialize;
use std::collections::HashSet;
use std::fmt::Debug;
use std::fmt::Display;
use streaming_iterator::StreamingIterator;
use tracing::info;
use tree_sitter::{Node, Query, QueryCursor, Tree};

use super::*;

#[cfg(feature = "ollama")]
use schemars::JsonSchema;

#[cfg(feature = "gemini")]
use {
    gemini::{Described, ResponseSchema, ToResponseSchema},
    gemini_macro_derive::ToResponseSchema,
};

#[cfg_attr(feature = "gemini", derive(ToResponseSchema))]
#[cfg_attr(feature = "ollama", derive(JsonSchema))]
#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Hash)]
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
impl Fix for Clause {
    fn fix_syntax(
        &mut self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.tag
            .fix_syntax(system_classes, current_class, current_feature)
            && self
                .predicate
                .fix_syntax(system_classes, current_class, current_feature)
    }

    fn fix_identifiers(
        &mut self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.tag
            .fix_identifiers(system_classes, current_class, current_feature)
            && self
                .predicate
                .fix_identifiers(system_classes, current_class, current_feature)
    }

    fn fix_calls(
        &mut self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.tag
            .fix_calls(system_classes, current_class, current_feature)
            && self
                .predicate
                .fix_calls(system_classes, current_class, current_feature)
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
            .map(|predicate| Predicate::new(node_to_text(&predicate, src)))
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

#[cfg_attr(feature = "gemini", derive(ToResponseSchema))]
#[cfg_attr(feature = "ollama", derive(JsonSchema))]
#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash)]
#[serde(transparent)]
pub struct Tag(String);

impl Tag {
    pub fn new<T: ToString>(text: T) -> Tag {
        Tag(text.to_string())
    }
    fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Tag {
    fn default() -> Self {
        Self(String::from("default"))
    }
}

impl Fix for Tag {
    fn fix_syntax(
        &mut self,
        _system_classes: &[&Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> bool {
        self.0 = self.0.to_lowercase().replace(" ", "_");
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

#[cfg_attr(feature = "gemini", derive(ToResponseSchema))]
#[cfg_attr(feature = "ollama", derive(JsonSchema))]
#[derive(Hash, Deserialize, Debug, PartialEq, Eq, Clone)]
#[serde(transparent)]
pub struct Predicate(String);

impl Predicate {
    pub fn new<T: ToString>(text: T) -> Predicate {
        Predicate(text.to_string())
    }
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
                let arg = node
                    .utf8_text(text.as_bytes())
                    .expect("valid capture for call's argument.");
                if !arg.is_empty() {
                    args.push(arg)
                }
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

impl Fix for Predicate {
    fn fix_syntax(
        &mut self,
        _system_classes: &[&Class],
        _current_class: &Class,
        _current_feature: &Feature,
    ) -> bool {
        match self.parse() {
            Some(tree) => !tree.root_node().has_error(),
            None => {
                info!("fails to parse predicate: {}", self.as_str());
                false
            }
        }
    }

    fn fix_identifiers(
        &mut self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.top_level_identifiers().iter().all(|&identifier| {
            current_class
                .features()
                .iter()
                .map(|feature| std::borrow::Cow::Borrowed(feature))
                .chain(current_class.inhereted_features(system_classes))
                .any(|feature| {
                    current_feature
                        .parameters()
                        .names()
                        .iter()
                        .any(|name| name == identifier)
                        || (identifier == feature.name())
                })
        })
    }

    /// NOTE: For now only checks the number of arguments of each unqualified call is correct.
    fn fix_calls(
        &mut self,
        system_classes: &[&Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.top_level_calls_with_arguments()
            .iter()
            .all(|&(id, ref args)| {
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

#[cfg(feature = "gemini")]
impl Described for Clause {
    fn description() -> String {
        String::from("A valid contract clause of the eiffel programming language.")
    }
}
#[cfg(feature = "gemini")]
impl Described for Tag {
    fn description() -> String {
        "A valid tag clause for the Eiffel programming language.".to_string()
    }
}
#[cfg(feature = "gemini")]
impl Described for Predicate {
    fn description() -> String {
        "A valid boolean expression for the Eiffel programming language.".to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::Result;
    #[cfg(feature = "gemini")]
    use gemini::SchemaType;

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

    // BEGIN: For gemini completions.
    // When the LSP grows in maturity, gemini will be decoupled and these tests will be moved to a compatibility layer.
    #[cfg(feature = "gemini")]
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
    #[cfg(feature = "gemini")]
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
    #[cfg(feature = "gemini")]
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
    // END: For gemini completion
    #[test]
    fn fix_predicate_syntax() {
        let src = "
            class
                A
            feature
                x (f: BOOLEAN, r: BOOLEAN): BOOLEAN
                    require
                        t: f = True
                    do
                        Result := f
                    ensure
                        res: Result = True
                    end
            end
        ";
        let c = Class::from_source(src);
        let f = c.features().first().unwrap();
        let sc = vec![&c];

        let mut invalid_predicate = Predicate::new("min min");
        let mut valid_predicate = Predicate::new("min (x, y)");
        assert!(!invalid_predicate.fix_syntax(&sc, &c, f));
        assert!(valid_predicate.fix_syntax(&sc, &c, f));
        assert_eq!(valid_predicate, Predicate::new("min (x, y)"));
    }
    #[test]
    fn fix_tag_syntax() {
        let src = "
            class
                A
            feature
                x (f: BOOLEAN, r: BOOLEAN): BOOLEAN
                    require
                        t: f = True
                    do
                        Result := f
                    ensure
                        res: Result = True
                    end
            end
        ";
        let c = Class::from_source(src);
        let f = c.features().first().unwrap();
        let sc = vec![&c];

        let mut invalid_tag: Tag = String::from("this was not valid").into();
        let mut valid_tag: Tag = String::from("this_is_valid").into();
        assert!(invalid_tag.fix_syntax(&sc, &c, f));
        assert!(invalid_tag == Tag::new("this_was_not_valid"));
        assert!(valid_tag.fix_syntax(&sc, &c, f));
        assert!(valid_tag == Tag::new("this_is_valid"));
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
    fn fix_predicates_identifiers() {
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
        let mut invalid_predicate = Predicate(String::from("z"));
        let mut valid_predicate = Predicate(String::from("x"));

        assert!(!invalid_predicate.fix_identifiers(&system_classes, &class, feature));
        assert!(valid_predicate.fix_identifiers(&system_classes, &class, feature));
    }
    #[test]
    fn fix_identifiers_predicate_ancestor() {
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
        let mut valid_predicate = Predicate(String::from("x"));
        assert!(valid_predicate.fix_identifiers(&system_classes, &child, feature));
    }
    #[test]
    fn fix_identifiers_in_predicate() {
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
        let mut vp = Predicate::new("f".to_string());
        let mut ip = Predicate::new("r".to_string());
        let system_classes = vec![&c];
        assert!(vp.fix_identifiers(&system_classes, &c, f));
        assert!(!ip.fix_identifiers(&system_classes, &c, f));
    }
    #[test]
    fn fix_calls_in_predicate() {
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

        let mut vp = Predicate::new("x (z)".to_string());
        let mut ip = Predicate::new("x (z, z)".to_string());
        let mut ip2 = Predicate::new("x ()".to_string());

        assert!(vp.fix_calls(&system_classes, &c, f));
        assert!(!ip.fix_calls(&system_classes, &c, f));
        assert!(!ip2.fix_calls(&system_classes, &c, f));
    }
    #[test]
    fn fix_tag() {
        let src = "
            class
                A
            feature
                x (f: BOOLEAN, r: BOOLEAN): BOOLEAN
                    require
                        t: f = True
                    do
                        Result := f
                    ensure
                        res: Result = True
                    end
            end
        ";
        let c = Class::from_source(src);
        let f = c.features().first().unwrap();
        let sc = vec![&c];

        let mut tag = Tag("Not good enough".to_string());
        assert!(tag.fix(&sc, &c, &f));

        assert_eq!(
            tag,
            Tag("not_good_enough".to_string()),
            "tag is:\t{tag}\nBut it must be:\t`not_good_enough`"
        )
    }
}
