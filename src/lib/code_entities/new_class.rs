use super::prelude::*;
use crate::lib::code_entities::class::model::*;
use crate::lib::code_entities::new_feature::FeatureID;
use crate::lib::code_entities::new_feature::FeatureName;
use crate::lib::code_entities::new_feature::Features;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::ops::Deref;

#[derive(PartialOrd, Ord, Debug, PartialEq, Eq, Copy, Clone, Hash, Default)]
pub struct ClassID(usize);

#[derive(PartialOrd, Ord, Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct ClassName(String);
impl Deref for ClassName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub(super) struct Parents {
    conformant: Vec<ClassName>,
    nonconformant: Vec<ClassName>,
    selects: Vec<FeatureName>,
    redefines: Vec<FeatureName>,
    undefines: Vec<FeatureName>,
    renames: BTreeMap<ClassName, BTreeMap<FeatureName, FeatureName>>,
}

impl<'slf> Parents {
    fn names(&self) -> impl Iterator<Item = &ClassName> {
        self.conformant.iter().chain(self.nonconformant.iter())
    }

    fn simple_iter(&self) -> impl Iterator<Item = (&ClassName, &Self)> {
        self.names().map(move |name| (name, self))
    }

    fn parent_ids(&'slf self, classes: &'slf Classes) -> impl Iterator<Item = &'slf ClassID> {
        self.names().map(move |name| classes.from_name(name))
    }

    fn rename_maps(&self, parent_id: &ClassName) -> Option<&BTreeMap<FeatureName, FeatureName>> {
        self.renames.get(parent_id)
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Classes {
    to_names: HashMap<ClassID, ClassName>,
    from_names: HashMap<ClassName, ClassID>,
    locations: HashMap<ClassID, Location>,
    features: HashMap<ClassID, Box<[FeatureID]>>,
    models: HashMap<ClassID, Model>,
    ranges: HashMap<ClassID, Range>,
    parents: HashMap<ClassID, Parents>,
}

impl<'slf> Classes {
    fn name_of(&self, id: &ClassID) -> &ClassName {
        self.to_names
            .get(id)
            .unwrap_or_else(|| panic!("fails to find name for class with id: {id:#?}"))
    }

    fn from_name(&self, name: &ClassName) -> &ClassID {
        self.from_names
            .get(name)
            .unwrap_or_else(|| panic!("fails to find id for class named: {name:#?}"))
    }

    fn parents(
        &'slf self,
        id: &'slf ClassID,
    ) -> impl Iterator<Item = (&'slf ClassName, &'slf Parents)> {
        self.parents
            .get(id)
            .into_iter()
            .flat_map(|parents| parents.simple_iter())
    }

    fn inheritance_renames_of(
        &'slf self,
        features: &'slf Features,
        id: &'slf ClassID,
    ) -> HashMap<&'slf FeatureID, &'slf FeatureName> {
        self.parents(id)
            .filter_map(|(name, parents)| parents.renames.get(name).map(|rename| (name, rename)))
            .map(|(name, renames)| {
                let id = self.from_name(name);
                let local_renames = self
                    .features
                    .get(id)
                    .into_iter()
                    .flatten()
                    .filter_map(|id| {
                        let old_name = features.name_of(id);
                        renames.get(old_name).map(|new_name| (id, new_name))
                    });
                let mut renames = self.inheritance_renames_of(features, id);
                renames.extend(local_renames);
                renames
            })
            .reduce(|mut acc, renames| {
                acc.extend(renames);
                acc
            })
            .unwrap_or_default()
    }

    fn inherited_feature_ids(
        &'slf self,
        features: &'slf Features,
        id: &'slf ClassID,
    ) -> Vec<&'slf FeatureID> {
        self.parents(id)
            .flat_map(|(parent_name, _)| {
                let id = self.from_name(parent_name);

                self.features
                    .get(id)
                    .into_iter()
                    .flatten()
                    .chain(self.inherited_feature_ids(features, id).into_iter())
            })
            .collect()
    }

    fn features(&self, id: &ClassID) -> Option<impl Iterator<Item = &FeatureID>> {
        self.features.get(id).map(|fts| fts.iter())
    }
}

struct Class<'cls, 'fts> {
    classes: &'cls Classes,
    features: &'fts Features,
    id: &'cls ClassID,
}

impl<'cls, 'fts> Class<'cls, 'fts> {
    fn with_id(&self, id: &'cls ClassID) -> Self {
        Self {
            id,
            classes: self.classes,
            features: self.features,
        }
    }
    fn current_features(&self) -> Option<impl Iterator<Item = &FeatureID>> {
        self.classes.features(&self.id)
    }

    fn immediate_or_inherited_features_ids(
        &self,
    ) -> Box<dyn Iterator<Item = &'cls FeatureID> + '_> {
        let immediate_features = self.classes.features.get(&self.id).map(|fts| fts.iter());
        let inherited_features = self.classes.parents.get(&self.id).map(
            |Parents {
                 conformant,
                 nonconformant,
                 ..
             }| {
                conformant
                    .iter()
                    .chain(nonconformant.iter())
                    .filter_map(|name| {
                        let id = self.classes.from_name(name);
                        self.classes.features.get(id)
                    })
                    .flat_map(|fts| fts.iter())
            },
        );
        match (immediate_features, inherited_features) {
            (Some(it_loc), Some(it_inh)) => Box::new(it_loc.chain(it_inh)),
            (Some(it_loc), None) => Box::new(it_loc),
            (None, Some(iter_inherited)) => Box::new(iter_inherited),
            (None, None) => Box::new(std::iter::empty()),
        }
    }
}
