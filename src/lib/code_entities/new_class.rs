use super::prelude::*;
use crate::lib::code_entities::class::model::*;
use crate::lib::code_entities::new_feature::FeatureID;
use crate::lib::code_entities::new_feature::Features;
use std::collections::HashMap;

mod new_ancestor;
use new_ancestor::Parents;

#[derive(PartialOrd, Ord, Debug, PartialEq, Eq, Copy, Clone, Hash, Default)]
pub struct ClassID(usize);

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct ClassLocalModel(ClassID, Model);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ClassParents(ClassID, Parents);

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct ClassRange(ClassID, Range);

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Classes {
    names: HashMap<ClassID, String>,
    locations: HashMap<ClassID, Location>,
    features: HashMap<ClassID, Box<[FeatureID]>>,
    models: HashMap<ClassID, Model>,
    ranges: HashMap<ClassID, Range>,
    parents: HashMap<ClassID, Parents>,
}

struct Class<'cls, 'fts> {
    classes: &'cls Classes,
    features: &'fts Features,
    id: ClassID,
}

impl<'cls, 'fts> Class<'cls, 'fts> {
    fn inherited_features_ids(
        &self,
    ) -> Option<impl Iterator<Item = &'fts FeatureID> + use<'fts, 'cls>> {
        self.classes.parents.get(&self.id).map(|parents| {
            parents
                .conformant
                .iter()
                .chain(parents.nonconformant.iter())
                .map(|cl_id| self.features.class_features(*cl_id))
                .flatten()
        })
    }

    // fn names_inherited_features(
    //     &self,
    // ) -> Option<impl Iterator<Item = &'fts str> + use<'fts, 'cls>> {
    //     todo!()
    // }
}

impl Classes {}
