use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::Debug;

use super::clause::Clause;
use super::*;

mod postcondition;
mod precondition;
pub use postcondition::Postcondition;
pub use precondition::Precondition;

#[derive(Debug, PartialEq, Eq, Clone, Hash, Deserialize, JsonSchema)]
#[schemars(deny_unknown_fields)]
#[schemars(
    description = "Hoare-style specifications of a given feature as preconditions and postconditions for AutoProof, Eiffel's static verifier."
)]
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
        if self.is_empty() {
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

#[cfg(test)]
mod tests {
    use super::super::clause::Predicate;
    use super::super::clause::Tag;
    use super::*;
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

        let mut vpr: Precondition =
            (vec![Clause::new(Tag::new("q"), Predicate::new("f = r"))]).into();
        let mut ipr: Precondition =
            (vec![Clause::new(Tag::new("s"), Predicate::new("f = True"))]).into();
        let mut ipr2: Precondition = (vec![
            Clause::new(Tag::new("qq"), Predicate::new("f = r")),
            Clause::new(Tag::new("q"), Predicate::new("f = r")),
        ])
        .into();

        let mut vpo: Postcondition =
            (vec![Clause::new(Tag::new("q"), Predicate::new("Result = f"))]).into();
        let mut ipo: Postcondition =
            (vec![Clause::new(Tag::new("t"), Predicate::new("Result = True"))]).into();
        let mut ipo2: Postcondition = (vec![
            Clause::new(Tag::new("qq"), Predicate::new("Result = f")),
            Clause::new(Tag::new("q"), Predicate::new("Result = f")),
        ])
        .into();

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
