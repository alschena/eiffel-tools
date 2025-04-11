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
    type Context;
    fn fixing_calls_of(&mut self, value: T, context: &Self::Context) -> Result<T>;
    fn fixing_identifiers_of(&mut self, value: T, context: &Self::Context) -> Result<T>;
    fn fixing_redundancy_of(&mut self, value: T, context: &Self::Context) -> Result<T>;
    fn fixing_syntax_of(&mut self, value: T, context: &Self::Context) -> Result<T>;
}

pub struct Context<'system> {
    system_classes: &'system [Class],
    current_class: &'system Class,
    current_feature: &'system Feature,
}

impl<'system> Fix<'system, ClauseTag> for Parser {
    type Context = Context<'system>;

    fn fixing_calls_of(&mut self, value: ClauseTag, _context: &Self::Context) -> Result<ClauseTag> {
        Ok(value)
    }

    fn fixing_identifiers_of(
        &mut self,
        value: ClauseTag,
        _context: &Self::Context,
    ) -> Result<ClauseTag> {
        Ok(value)
    }

    fn fixing_redundancy_of(
        &mut self,
        value: ClauseTag,
        _context: &Self::Context,
    ) -> Result<ClauseTag> {
        Ok(value)
    }

    fn fixing_syntax_of(
        &mut self,
        mut value: ClauseTag,
        context: &Self::Context,
    ) -> Result<ClauseTag> {
        value.update_to_lowercase();
        value.trim_and_replace_space_with_underscore();
        Ok(value)
    }
}

impl<'system> Fix<'system, ClausePredicate> for Parser {
    type Context = Context<'system>;

    fn fixing_calls_of(
        &mut self,
        value: ClausePredicate,
        context: &Self::Context,
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
                all_current_class_features_names_and_number_of_parameters
                    .contains(&(id.as_str(), args.len()))
            })
            .collect::<Vec<_>>();

        ensure!(
            invalid_top_level_call_identifiers.is_empty(),
            "fails to fix invalid topo level call identifiers"
        );

        Ok(value)
    }

    fn fixing_identifiers_of(
        &mut self,
        value: ClausePredicate,
        context: &Self::Context,
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
            .iter()
            .filter(|&id| feature_names.contains(id));

        let invalid_identifiers = identifiers_other_than_feature_names
            .filter(|&id| parameters_names.contains(id))
            .collect::<Vec<_>>();

        ensure!(
            invalid_identifiers.is_empty(),
            "fails to fix the current predicate's top level identifiers"
        );

        Ok(value)
    }

    fn fixing_redundancy_of(
        &mut self,
        value: ClausePredicate,
        _context: &Self::Context,
    ) -> Result<ClausePredicate> {
        Ok(value)
    }

    fn fixing_syntax_of(
        &mut self,
        value: ClausePredicate,
        _context: &Self::Context,
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
        fn $name(&mut self, value: Clause, context: &Self::Context) -> Result<Clause> {
            let Clause { tag, predicate } = value;
            let tag = self.$name(tag, context)?;
            let predicate = self.$name(predicate, context)?;
            Ok(Clause { tag, predicate })
        }
    };
}

impl<'system> Fix<'system, Clause> for Parser {
    type Context = Context<'system>;

    clause_defaul_impl!(fixing_calls_of);
    clause_defaul_impl!(fixing_identifiers_of);
    clause_defaul_impl!(fixing_redundancy_of);
    clause_defaul_impl!(fixing_syntax_of);
}

macro_rules! precondition_default_impl {
    ($name:ident) => {
        fn $name(&mut self, value: Precondition, context: &Self::Context) -> Result<Precondition> {
            let Precondition(clauses) = value;
            let new_clauses = clauses
                .into_iter()
                .filter_map(|clause| self.$name(clause, context).ok())
                .collect();
            Ok(Precondition(new_clauses))
        }
    };
}

impl<'system> Fix<'system, Precondition> for Parser {
    type Context = Context<'system>;

    precondition_default_impl!(fixing_calls_of);
    precondition_default_impl!(fixing_identifiers_of);
    precondition_default_impl!(fixing_redundancy_of);
    precondition_default_impl!(fixing_syntax_of);
}

macro_rules! postcondition_default_impl {
    ($name:ident) => {
        fn $name(
            &mut self,
            value: Postcondition,
            context: &Self::Context,
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
    type Context = Context<'system>;

    postcondition_default_impl!(fixing_calls_of);
    postcondition_default_impl!(fixing_identifiers_of);
    postcondition_default_impl!(fixing_redundancy_of);
    postcondition_default_impl!(fixing_syntax_of);
}

macro_rules! routine_specification_default_impl {
    ($name:ident) => {
        fn $name(
            &mut self,
            value: RoutineSpecification,
            context: &Self::Context,
        ) -> Result<RoutineSpecification> {
            let RoutineSpecification {
                precondition,
                postcondition,
            } = value;
            let precondition = self.$name(precondition, context)?;
            let postcondition = self.$name(postcondition, context)?;
            Ok(RoutineSpecification {
                precondition,
                postcondition,
            })
        }
    };
}

impl<'system> Fix<'system, RoutineSpecification> for Parser {
    type Context = Context<'system>;

    routine_specification_default_impl!(fixing_calls_of);
    routine_specification_default_impl!(fixing_identifiers_of);
    routine_specification_default_impl!(fixing_redundancy_of);
    routine_specification_default_impl!(fixing_syntax_of);
}
