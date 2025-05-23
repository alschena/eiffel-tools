use crate::lib::code_entities::contract::Contract;
use crate::lib::code_entities::prelude::contract::Clause;
use crate::lib::code_entities::prelude::contract::ClausePredicate;
use crate::lib::code_entities::prelude::contract::ClauseTag;
use crate::lib::code_entities::prelude::contract::Postcondition;
use crate::lib::code_entities::prelude::contract::Precondition;
use crate::lib::code_entities::prelude::contract::RoutineSpecification;
use crate::lib::code_entities::prelude::Class;
use crate::lib::code_entities::prelude::Feature;
use crate::lib::parser::ExpressionTree;
use crate::lib::parser::Parser;
use anyhow::bail;
use anyhow::ensure;
use anyhow::Result;
use std::collections::HashSet;

pub trait Fix<'system, T> {
    type PositionInSystem;
    fn fix(&mut self, value: T, position_in_system: &Self::PositionInSystem) -> Result<T> {
        let value = self.fix_syntax_of(value, position_in_system)?;
        let value = self.fix_calls_of(value, position_in_system)?;
        let value = self.fix_identifiers_of(value, position_in_system)?;
        let value = self.fix_redundancy_of(value, position_in_system)?;
        Ok(value)
    }
    fn fix_calls_of(&mut self, value: T, position_in_system: &Self::PositionInSystem) -> Result<T>;
    fn fix_identifiers_of(
        &mut self,
        value: T,
        position_in_system: &Self::PositionInSystem,
    ) -> Result<T>;
    fn fix_redundancy_of(
        &mut self,
        value: T,
        position_in_system: &Self::PositionInSystem,
    ) -> Result<T>;
    fn fix_syntax_of(&mut self, value: T, position_in_system: &Self::PositionInSystem)
        -> Result<T>;
}

pub struct FeaturePositionInSystem<'system> {
    system_classes: &'system [Class],
    current_class: &'system Class,
    current_feature: &'system Feature,
}
impl<'system> FeaturePositionInSystem<'system> {
    pub fn new(
        system_classes: &'system [Class],
        current_class: &'system Class,
        current_feature: &'system Feature,
    ) -> Self {
        Self {
            system_classes,
            current_class,
            current_feature,
        }
    }
}

impl<'system> Fix<'system, ClauseTag> for Parser {
    type PositionInSystem = FeaturePositionInSystem<'system>;

    fn fix_calls_of(
        &mut self,
        value: ClauseTag,
        _context: &Self::PositionInSystem,
    ) -> Result<ClauseTag> {
        Ok(value)
    }

    fn fix_identifiers_of(
        &mut self,
        value: ClauseTag,
        _context: &Self::PositionInSystem,
    ) -> Result<ClauseTag> {
        Ok(value)
    }

    fn fix_redundancy_of(
        &mut self,
        value: ClauseTag,
        _context: &Self::PositionInSystem,
    ) -> Result<ClauseTag> {
        Ok(value)
    }

    fn fix_syntax_of(
        &mut self,
        mut value: ClauseTag,
        _context: &Self::PositionInSystem,
    ) -> Result<ClauseTag> {
        value.update_to_lowercase();
        value.trim_and_replace_space_with_underscore();
        Ok(value)
    }
}

impl<'system> Fix<'system, ClausePredicate> for Parser {
    type PositionInSystem = FeaturePositionInSystem<'system>;

    fn fix_calls_of(
        &mut self,
        value: ClausePredicate,
        context: &Self::PositionInSystem,
    ) -> Result<ClausePredicate> {
        let parsed_source = self.parse(value.as_str())?;
        let mut expression_tree = parsed_source.expression_tree_traversal()?;

        let top_level_calls_with_arguments = expression_tree.top_level_calls_with_arguments()?;

        let immediate_and_inherited_features = context
            .current_class
            .immediate_and_inherited_features(context.system_classes);

        let all_current_class_features_names_and_number_of_parameters =
            immediate_and_inherited_features
                .iter()
                .map(|feature| (feature.name(), feature.number_parameters()))
                .collect::<HashSet<_>>();

        let invalid_top_level_call_identifiers = top_level_calls_with_arguments
            .iter()
            .filter(|&(id, ref args)| {
                eprintln!(
                    "filter eval: {:#?} for pair {:#?}. args {:#?}",
                    !all_current_class_features_names_and_number_of_parameters
                        .contains(&(id.as_str(), args.len())),
                    &(id.as_str(), args.len()),
                    args
                );
                !all_current_class_features_names_and_number_of_parameters
                    .contains(&(id.as_str(), args.len()))
            })
            .inspect(|val| eprintln!("val:{val:#?}"))
            .collect::<Vec<_>>();

        ensure!(
            invalid_top_level_call_identifiers.is_empty(),
            "fails to fix {value:#?} invalid top level call identifiers; remaining: {invalid_top_level_call_identifiers:#?}"
        );

        Ok(value)
    }

    fn fix_identifiers_of(
        &mut self,
        value: ClausePredicate,
        context: &Self::PositionInSystem,
    ) -> Result<ClausePredicate> {
        let parsed_source = self.parse(value.as_str())?;
        let mut expression_tree_traversal = parsed_source.expression_tree_traversal()?;

        let predicate_identifiers = expression_tree_traversal.top_level_identifiers()?;

        let current_class_inherited_features = context
            .current_class
            .immediate_and_inherited_features(context.system_classes);

        let feature_names: HashSet<_> = current_class_inherited_features
            .iter()
            .map(|feature| feature.name())
            .collect();

        let parameters_names: HashSet<_> = context
            .current_feature
            .parameters()
            .names()
            .into_iter()
            .map(|name| name.as_str())
            .collect();

        let identifiers_other_than_feature_names = predicate_identifiers
            .into_iter()
            .filter(|&id| !feature_names.contains(id));

        let invalid_identifiers = identifiers_other_than_feature_names
            .filter(|&id| !parameters_names.contains(id))
            .collect::<Vec<_>>();

        ensure!(
            invalid_identifiers.is_empty(),
            "fails to fix the current predicate's top level identifiers"
        );

        Ok(value)
    }

    fn fix_redundancy_of(
        &mut self,
        value: ClausePredicate,
        _context: &Self::PositionInSystem,
    ) -> Result<ClausePredicate> {
        Ok(value)
    }

    fn fix_syntax_of(
        &mut self,
        value: ClausePredicate,
        _context: &Self::PositionInSystem,
    ) -> Result<ClausePredicate> {
        let parsed_source = self.parse(value.as_str())?;
        if !parsed_source.tree.root_node().has_error() {
            Ok(value)
        } else {
            bail!("fails parsing expression.")
        }
    }
}

macro_rules! clause_defaul_impl {
    ($name:ident) => {
        fn $name(
            &mut self,
            value: Clause,
            position_in_system: &Self::PositionInSystem,
        ) -> Result<Clause> {
            let Clause { tag, predicate } = value;
            let tag = self.$name(tag, position_in_system)?;
            let predicate = self.$name(predicate, position_in_system)?;
            Ok(Clause { tag, predicate })
        }
    };
}

impl<'system> Fix<'system, Clause> for Parser {
    type PositionInSystem = FeaturePositionInSystem<'system>;

    clause_defaul_impl!(fix_calls_of);
    clause_defaul_impl!(fix_identifiers_of);
    clause_defaul_impl!(fix_redundancy_of);
    clause_defaul_impl!(fix_syntax_of);
}

macro_rules! precondition_default_impl {
    ($name:ident) => {
        fn $name(
            &mut self,
            value: Precondition,
            position_in_system: &Self::PositionInSystem,
        ) -> Result<Precondition> {
            let Precondition(clauses) = value;
            let new_clauses = clauses
                .into_iter()
                .filter_map(|clause| self.$name(clause, position_in_system).ok())
                .collect();
            Ok(Precondition(new_clauses))
        }
    };
}

impl<'system> Fix<'system, Precondition> for Parser {
    type PositionInSystem = FeaturePositionInSystem<'system>;

    precondition_default_impl!(fix_calls_of);
    precondition_default_impl!(fix_identifiers_of);
    precondition_default_impl!(fix_syntax_of);

    fn fix_redundancy_of(
        &mut self,
        value: Precondition,
        context: &Self::PositionInSystem,
    ) -> Result<Precondition> {
        let mut value = value;
        match context.current_feature.preconditions() {
            Some(pr) => value.remove_redundant_clauses(pr),
            None => value.remove_self_redundant_clauses(),
        }
        Ok(value)
    }
}

macro_rules! postcondition_default_impl {
    ($name:ident) => {
        fn $name(
            &mut self,
            value: Postcondition,
            context: &Self::PositionInSystem,
        ) -> Result<Postcondition> {
            let Postcondition(clauses) = value;
            let new_clauses = clauses
                .into_iter()
                .filter_map(|clause| self.$name(clause, context).ok())
                .collect();
            Ok(Postcondition(new_clauses))
        }
    };
}

impl<'system> Fix<'system, Postcondition> for Parser {
    type PositionInSystem = FeaturePositionInSystem<'system>;

    postcondition_default_impl!(fix_calls_of);
    postcondition_default_impl!(fix_identifiers_of);
    postcondition_default_impl!(fix_syntax_of);

    fn fix_redundancy_of(
        &mut self,
        value: Postcondition,
        position_in_system: &Self::PositionInSystem,
    ) -> Result<Postcondition> {
        let mut value = value;
        match position_in_system.current_feature.postconditions() {
            Some(pr) => value.remove_redundant_clauses(pr),
            None => value.remove_self_redundant_clauses(),
        }
        Ok(value)
    }
}

macro_rules! routine_specification_default_impl {
    ($name:ident) => {
        fn $name(
            &mut self,
            value: RoutineSpecification,
            position_in_system: &Self::PositionInSystem,
        ) -> Result<RoutineSpecification> {
            let RoutineSpecification {
                precondition,
                postcondition,
            } = value;
            let precondition = self.$name(precondition, position_in_system)?;
            let postcondition = self.$name(postcondition, position_in_system)?;
            Ok(RoutineSpecification {
                precondition,
                postcondition,
            })
        }
    };
}

impl<'system> Fix<'system, RoutineSpecification> for Parser {
    type PositionInSystem = FeaturePositionInSystem<'system>;

    routine_specification_default_impl!(fix_calls_of);
    routine_specification_default_impl!(fix_identifiers_of);
    routine_specification_default_impl!(fix_redundancy_of);
    routine_specification_default_impl!(fix_syntax_of);
}

#[cfg(test)]
mod tests {
    use anyhow::Context;

    use super::*;
    use crate::lib::code_entities::contract::ClauseTag;

    #[test]
    fn fix_tag() -> Result<()> {
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
        let mut parser = Parser::new();
        let system_classes = &vec![parser.class_from_source(src)?];
        let current_class = &system_classes[0];
        let current_feature = current_class.features().first().unwrap();

        let tag = ClauseTag::new("Not good enough");
        let fixing_context = FeaturePositionInSystem {
            system_classes,
            current_class,
            current_feature,
        };
        let fixed_tag = parser
            .fix(tag, &fixing_context)
            .inspect_err(|e| eprintln!("fails to fix tag with error: {e}"))?;

        assert_eq!(
            fixed_tag,
            ClauseTag::new("not_good_enough"),
            "fixed tag is:\t{fixed_tag}\nBut it must be:\t`not_good_enough`"
        );
        Ok(())
    }

    #[test]
    fn fix_predicate_syntax() -> Result<()> {
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
        let mut parser = Parser::new();
        let system_classes = &vec![parser.class_from_source(src)?];
        let current_class = &system_classes[0];
        let current_feature = current_class.features().first().unwrap();

        let invalid_predicate = ClausePredicate::new("min min");
        let valid_predicate = ClausePredicate::new("min (x, y)");

        let fixing_context = FeaturePositionInSystem {
            system_classes,
            current_class,
            current_feature,
        };

        let fix_invalid_predicate = parser
            .fix_syntax_of(invalid_predicate, &fixing_context)
            .inspect(|val| eprintln!("fails to fail, instead returns: {val}"));
        let fix_valid_predicate = parser
            .fix_syntax_of(valid_predicate, &fixing_context)
            .inspect_err(|e| eprintln!("fails to accept valid predicate with error: {e}"))?;

        assert!(fix_invalid_predicate.is_err());
        assert_eq!(fix_valid_predicate, ClausePredicate::new("min (x, y)"));
        Ok(())
    }

    #[test]
    fn fix_predicates_identifiers() -> anyhow::Result<()> {
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
        let mut parser = Parser::new();
        let system_classes = &vec![parser.class_from_source(src)?];
        let current_class = &system_classes[0];
        let current_feature = current_class
            .features()
            .iter()
            .find(|f| f.name() == "y".to_string())
            .expect("parse feature y");

        let invalid_predicate = ClausePredicate::new("z");
        let valid_predicate = ClausePredicate::new("x");

        let fixing_context = FeaturePositionInSystem {
            system_classes,
            current_class,
            current_feature,
        };

        let fixed_invalid_predicate = parser
            .fix_identifiers_of(invalid_predicate, &fixing_context)
            .inspect(|val| eprintln!("fails to refuse invalid predicate, instead returns: {val}"));
        let fixed_valid_predicate = parser
            .fix_identifiers_of(valid_predicate.clone(), &fixing_context)
            .inspect_err(|e| {
                eprintln!(
                    "fails to accept valid predicate: {valid_predicate:#?} with error: {e:#?}"
                )
            })?;

        assert!(fixed_invalid_predicate.is_err());
        assert_eq!(fixed_valid_predicate, valid_predicate);
        Ok(())
    }

    #[test]
    fn fix_identifiers_in_predicate() -> Result<()> {
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
        let mut parser = Parser::new();
        let system_classes = &vec![parser.class_from_source(src)?];
        let current_class = &system_classes[0];
        let current_feature = current_class
            .features()
            .first()
            .expect("first feature exists.");
        let vp = ClausePredicate::new("f");
        let ip = ClausePredicate::new("r");

        let fixing_context = FeaturePositionInSystem {
            system_classes,
            current_class,
            current_feature,
        };
        let fixed_vp = parser
            .fix_identifiers_of(vp.clone(), &fixing_context)
            .inspect_err(|e| {
                eprintln!("fails to accept valid predicate: {vp:#?}, fails with error: {e:#?}")
            })?;
        let fixed_ip = parser
            .fix_identifiers_of(ip, &fixing_context)
            .inspect(|val| eprintln!("fails to fail, instead returns: {val:#?}"));

        assert_eq!(fixed_vp, vp);
        assert!(fixed_ip.is_err());
        Ok(())
    }

    #[test]
    fn fix_ancestor_identifiers_predicate() -> anyhow::Result<()> {
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

        let mut parser = Parser::new();
        let system_classes = &vec![
            parser.class_from_source(parent_src)?,
            parser.class_from_source(child_src)?,
        ];
        let current_class = &system_classes[1];
        let current_feature = current_class
            .features()
            .iter()
            .find(|f| f.name() == "y")
            .expect("parse feature y");

        assert!(
            current_class
                .features()
                .into_iter()
                .find(|f| f.name() == "x")
                .is_none(),
            "ensure the feature `x` is inherited."
        );

        let valid_predicate = ClausePredicate::new("x");

        let fixing_context = FeaturePositionInSystem {
            system_classes,
            current_class,
            current_feature,
        };
        let fixed_valid_predicate = parser
            .fix_identifiers_of(valid_predicate.clone(), &fixing_context)
            .inspect_err(|e| {
                eprintln!("fails to accept valid inherited identifier with error: {e:#?}")
            })?;

        assert_eq!(fixed_valid_predicate, valid_predicate);
        Ok(())
    }

    #[test]
    fn fix_calls_in_predicate() -> Result<()> {
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
        eprintln!("source: {src}");
        let mut parser = Parser::new();
        let system_classes = &vec![parser.class_from_source(src)?];
        let current_class = &system_classes[0];
        let current_feature = current_class
            .features()
            .iter()
            .find(|f| f.name() == "y")
            .expect("first feature exists.");

        let vp = ClausePredicate::new("x (z)");
        let ip = ClausePredicate::new("x (z, z)");
        let ip2 = ClausePredicate::new("x ()");

        let fixing_context = FeaturePositionInSystem {
            system_classes,
            current_class,
            current_feature,
        };

        let fixed_vp = parser.fix_calls_of(vp, &fixing_context)?;
        let fixed_ip = parser.fix_calls_of(ip, &fixing_context).inspect(|val| {
            eprintln!("fails because it was supposed to return error instead of the value: {val}")
        });
        let fixed_ip2 = parser.fix_calls_of(ip2, &fixing_context).inspect(|val| {
            eprintln!("fails because it was supposed to return error instead of the value: {val}")
        });

        assert_eq!(fixed_vp, ClausePredicate::new("x (z)"));
        assert!(fixed_ip.is_err());
        assert!(fixed_ip2.is_err());

        Ok(())
    }

    #[test]
    fn fix_precondition_repetition() -> Result<()> {
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
        let mut parser = Parser::new();
        let system_classes = &vec![parser.class_from_source(src)?];
        let current_class = &system_classes[0];
        let current_feature = current_class.features().first().unwrap();

        let context = FeaturePositionInSystem {
            system_classes,
            current_class,
            current_feature,
        };

        let pre: Precondition = vec![
            Clause::from_line("s: f = r").with_context(|| "fails to create clause from line")?,
            Clause::from_line("ss: f = r").with_context(|| "fails to create clause from line")?,
        ]
        .into();

        let fixed_pre = parser.fix(pre, &context)?;
        assert!(fixed_pre
            .first()
            .is_some_and(|p| p.predicate == ClausePredicate::new("f = r")));
        Ok(())
    }

    #[test]
    fn fix_routine_specification_repetition() -> Result<()> {
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
        let mut parser = Parser::new();
        let system_classes = vec![parser.class_from_source(src)?];
        let c = &system_classes[0];
        let f = c.features().first().unwrap();

        let precondition_contract_clause =
            Clause::from_line("q: f = r").expect(r#"fails to get clause from "q: f = r""#);
        let precondition_contract_clause_different_tag =
            Clause::from_line("qq: f = r").expect(r#"fails to get clause from "qq: f = r""#);
        let redundant_precondition_contract_clause =
            Clause::from_line("s: f = True").expect(r#"fails to get clause from "s: f = True""#);

        let vpr: Precondition = (vec![precondition_contract_clause.clone()]).into();
        let ipr: Precondition = (vec![redundant_precondition_contract_clause]).into();
        let ipr2: Precondition = (vec![
            precondition_contract_clause_different_tag,
            precondition_contract_clause,
        ])
        .into();

        let postcondition_contract_clause = Clause::from_line("q: Result = f")
            .expect(r#"fails to get clause from "q: Result = f""#);
        let postcondition_contract_clause_different_tag = Clause::from_line("qq: Result = f")
            .expect(r#"fails to get clause from "qq: Result = f""#);
        let redundant_postcondition_contract_clause = Clause::from_line("s: Result = True")
            .expect(r#"fails to get clause from "s: Result = True""#);

        let vpo: Postcondition = (vec![postcondition_contract_clause.clone()]).into();
        let ipo: Postcondition = (vec![redundant_postcondition_contract_clause]).into();
        let ipo2: Postcondition = (vec![
            postcondition_contract_clause_different_tag,
            postcondition_contract_clause,
        ])
        .into();

        eprintln!("preconditions: {}", f.preconditions().unwrap());
        eprintln!("postconditions: {}", f.postconditions().unwrap());

        let feature_position_in_system = FeaturePositionInSystem {
            system_classes: &system_classes,
            current_class: c,
            current_feature: f,
        };

        let fixed_vpr = parser.fix(vpr, &feature_position_in_system)?;
        let fixed_ipr = parser.fix(ipr, &feature_position_in_system)?;
        let fixed_ipr2 = parser.fix(ipr2, &feature_position_in_system)?;

        assert!(
            fixed_ipr.is_empty(),
            "fixing `s: f = True` should return empty instead of: {fixed_ipr:#?}"
        );
        assert_eq!(fixed_ipr2, fixed_vpr);

        let fixed_vpo = parser.fix(vpo, &feature_position_in_system)?;
        let fixed_ipo = parser.fix(ipo, &feature_position_in_system)?;
        let fixed_ipo2 = parser.fix(ipo2, &feature_position_in_system)?;

        assert!(fixed_ipo.is_empty());
        assert_eq!(fixed_ipo2, fixed_vpo);
        Ok(())
    }
}
