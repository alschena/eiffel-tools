use crate::lib::tree_sitter_extension::Parse;
use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::Debug;
use std::fmt::Display;
use std::ops::Deref;
use std::ops::DerefMut;
use streaming_iterator::StreamingIterator;
use tree_sitter::Node;
use tree_sitter::QueryCursor;

use super::clause::Clause;
use super::*;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
/// Wraps an optional contract clause adding whereabouts informations.
/// If the `item` is None, the range start and end coincide where the contract clause would be added.
pub struct Block<T> {
    pub item: T,
    pub range: Range,
}
impl<T: Contract> Block<T> {
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
impl<T: Contract + Default> Block<T> {
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
impl<T: Display + Indent + Contract + Deref<Target = Vec<Clause>>> Display for Block<T> {
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
#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Hash, JsonSchema)]
#[serde(transparent)]
#[schemars(deny_unknown_fields)]
pub struct Precondition(Vec<Clause>);

impl Precondition {
    fn redundant_clauses_wrt_feature<'a>(
        &self,
        feature: &'a Feature,
    ) -> impl Iterator<Item = (usize, &Clause)> + use<'_, 'a> {
        self.iter().enumerate().filter(|(n, c)| {
            self.iter()
                .skip(n + 1)
                .any(|nc| &nc.predicate == &c.predicate)
                || feature
                    .preconditions()
                    .is_some_and(|pre| pre.iter().any(|nc| &nc.predicate == &c.predicate))
        })
    }
}

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

impl Fix for Precondition {
    fn fix_syntax(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.retain_mut(|clause| clause.fix_syntax(system_classes, current_class, current_feature));
        true
    }
    fn fix_identifiers(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.retain_mut(|clause| {
            clause.fix_identifiers(system_classes, current_class, current_feature)
        });
        true
    }
    fn fix_calls(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.retain_mut(|clause| clause.fix_calls(system_classes, current_class, current_feature));
        true
    }
    fn fix_repetition(
        &mut self,
        _system_classes: &[Class],
        _current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        match current_feature.preconditions() {
            Some(pr) => self.remove_redundant_clauses(pr),
            None => self.remove_self_redundant_clauses(),
        }
        true
    }
}
impl From<Vec<Clause>> for Precondition {
    fn from(value: Vec<Clause>) -> Self {
        Self(value)
    }
}
impl Contract for Precondition {
    fn keyword() -> Keyword {
        Keyword::Require
    }
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

impl Precondition {
    fn description() -> String {
        "Preconditions are predicates on the prestate, the state before the execution, of a routine. They describe the properties that the fields of the model in the current object must satisfy in the prestate. Preconditions cannot contain a call to `old_` or the `old` keyword.".to_string()
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
#[derive(Hash, Deserialize, Debug, PartialEq, Eq, Clone, JsonSchema)]
#[serde(transparent)]
#[schemars(deny_unknown_fields)]
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

impl Contract for Postcondition {
    fn keyword() -> Keyword {
        Keyword::Ensure
    }
}

impl From<Vec<Clause>> for Postcondition {
    fn from(value: Vec<Clause>) -> Self {
        Self(value)
    }
}

impl Fix for Postcondition {
    fn fix_syntax(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.retain_mut(|clause| clause.fix_syntax(system_classes, current_class, current_feature));
        true
    }
    fn fix_identifiers(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.retain_mut(|clause| {
            clause.fix_identifiers(system_classes, current_class, current_feature)
        });
        true
    }
    fn fix_calls(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.retain_mut(|clause| clause.fix_calls(system_classes, current_class, current_feature));
        true
    }
    fn fix_repetition(
        &mut self,
        _system_classes: &[Class],
        _current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        match current_feature.postconditions() {
            Some(pos) => self.remove_redundant_clauses(pos),
            None => self.remove_self_redundant_clauses(),
        }
        true
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
impl Postcondition {
    fn description() -> String {
        "Postconditions describe the properties that the model of the current object must satisfy after the routine.
        Postconditions are two-states predicates.
        They can refer to the prestate of the routine by calling the feature `old_` on any object which existed before the execution of the routine.
        Equivalently, you can use the keyword `old` before a feature to access its prestate.".to_string()
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

#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
pub struct RoutineSpecification {
    pub precondition: Precondition,
    pub postcondition: Postcondition,
}

impl RoutineSpecification {
    pub fn is_empty(&self) -> bool {
        self.precondition.is_empty() && self.postcondition.is_empty()
    }
    pub fn from_markdown(markdown: &str) -> Self {
        let precondition: Precondition = markdown
            .lines()
            .skip_while(|line| !line.contains("# Pre"))
            .skip(1)
            .map_while(|line| {
                let line = line.trim();
                (!line.starts_with("# ")).then_some(Clause::from_line(line).or_else(|| {
                    info!("fail to parse the line:\t{line}\n");
                    None
                }))
            })
            .filter_map(|clause| clause)
            .collect::<Vec<_>>()
            .into();
        let postcondition: Postcondition = markdown
            .lines()
            .skip_while(|line| !line.contains("# Post"))
            .skip(1)
            .map_while(|line| {
                let line = line.trim();
                (!line.starts_with("# ")).then_some(Clause::from_line(line).or_else(|| {
                    info!("fail to parse the line:\t{line}\n");
                    None
                }))
            })
            .filter_map(|clause| clause)
            .collect::<Vec<_>>()
            .into();
        RoutineSpecification {
            precondition,
            postcondition,
        }
    }
}
impl Fix for RoutineSpecification {
    fn fix_syntax(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        if !self
            .precondition
            .fix_syntax(system_classes, current_class, current_feature)
        {
            info!(target:"llm", "fail fixing precondition");
            return false;
        }
        if !self
            .postcondition
            .fix_syntax(system_classes, current_class, current_feature)
        {
            info!(target:"llm", "fail fixing postcondition.");
            return false;
        }
        if !self.precondition.is_empty() && !self.postcondition.is_empty() {
            info!(target:"llm", "empty routine specification");
            return false;
        }
        true
    }
    fn fix_identifiers(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.precondition
            .fix_identifiers(system_classes, current_class, current_feature)
            && self
                .postcondition
                .fix_identifiers(system_classes, current_class, current_feature)
    }
    fn fix_calls(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.precondition
            .fix_calls(system_classes, current_class, current_feature)
            && self
                .postcondition
                .fix_calls(system_classes, current_class, current_feature)
    }
    fn fix_repetition(
        &mut self,
        system_classes: &[Class],
        current_class: &Class,
        current_feature: &Feature,
    ) -> bool {
        self.precondition
            .fix_repetition(system_classes, current_class, current_feature)
            && self
                .postcondition
                .fix_repetition(system_classes, current_class, current_feature)
    }
}
impl RoutineSpecification {
    fn description() -> String {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::super::clause::Predicate;
    use super::super::clause::Tag;
    use super::*;
    use anyhow::Result;

    #[test]
    fn fix_repetition_in_preconditions() {
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
        let sc = vec![Class::from_source(src)];
        let c = &sc[0];
        let f = c.features().first().unwrap();

        let mut fp = Precondition(vec![
            Clause::new(Tag::new("s"), Predicate::new("f = r")),
            Clause::new(Tag::new("ss"), Predicate::new("f = r")),
        ]);

        assert!(fp.fix(&sc, &c, f));
        assert!(fp
            .first()
            .is_some_and(|p| p.predicate == Predicate::new("f = r")))
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
        assert_eq!(*tag, Tag::new(""));
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
        assert_eq!(&clause.tag, &Tag::new(""));
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
    fn fix_routine_specification_wrt_repetition() {
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
        let system_classes = vec![Class::from_source(src)];
        let c = &system_classes[0];
        let f = c.features().first().unwrap();

        let mut vpr = Precondition(vec![Clause::new(Tag::new("q"), Predicate::new("f = r"))]);
        let mut ipr = Precondition(vec![Clause::new(Tag::new("s"), Predicate::new("f = True"))]);
        let mut ipr2 = Precondition(vec![
            Clause::new(Tag::new("qq"), Predicate::new("f = r")),
            Clause::new(Tag::new("q"), Predicate::new("f = r")),
        ]);

        let mut vpo = Postcondition(vec![Clause::new(
            Tag::new("q"),
            Predicate::new("Result = f"),
        )]);
        let mut ipo = Postcondition(vec![Clause::new(
            Tag::new("t"),
            Predicate::new("Result = True"),
        )]);
        let mut ipo2 = Postcondition(vec![
            Clause::new(Tag::new("qq"), Predicate::new("Result = f")),
            Clause::new(Tag::new("q"), Predicate::new("Result = f")),
        ]);

        eprintln!("preconditions: {}", f.preconditions().unwrap());
        eprintln!("postconditions: {}", f.postconditions().unwrap());

        assert!(
            vpr.fix(&system_classes, &c, f),
            "fixed preconditions: {vpr}",
        );
        assert!(
            ipr.fix(&system_classes, &c, f),
            "fixed preconditions: {ipr}"
        );
        assert!(ipr.is_empty());
        assert!(
            ipr2.fix(&system_classes, &c, f),
            "fixed preconditions: {ipr2}"
        );
        assert_eq!(ipr2, vpr);

        assert!(
            vpo.fix(&system_classes, &c, f),
            "fixed postconditions: {vpo}",
        );
        assert!(
            ipo.fix(&system_classes, &c, f),
            "fixed postconditions: {ipo}"
        );
        assert!(ipo.is_empty());
        assert!(
            ipo2.fix(&system_classes, &c, f),
            "fixed postconditions: {ipo2}",
        );
        assert_eq!(ipo2, vpo);
    }
}
