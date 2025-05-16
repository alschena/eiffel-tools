use super::prelude::*;
use crate::lib::code_entities::class::model::*;
use crate::lib::code_entities::new_feature;
use crate::lib::code_entities::new_feature::FeatureID;
use crate::lib::code_entities::new_feature::FeatureName;
use crate::lib::code_entities::new_feature::Features;
use std::borrow::Borrow;
use std::collections::BTreeMap;
use std::collections::HashMap;
use std::ops::Deref;

#[derive(PartialOrd, Ord, Debug, PartialEq, Eq, Copy, Clone, Hash, Default)]
pub struct ClassID(usize);
impl<T: Into<usize>> From<T> for ClassID {
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

#[derive(PartialOrd, Ord, Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct ClassName(String);

impl<T: ToString> From<T> for ClassName {
    fn from(value: T) -> Self {
        Self(value.to_string())
    }
}
impl Borrow<str> for ClassName {
    fn borrow(&self) -> &str {
        &self.0
    }
}
impl AsRef<str> for ClassName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}
impl Deref for ClassName {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(PartialOrd, Ord, Debug, PartialEq, Eq, Clone, Copy, Hash, Default)]
pub struct FeatureNumber(usize);

impl FeatureNumber {
    fn new<T: Into<usize>>(value: T) -> Self {
        Self(value.into())
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct Parents {
    conformant: Vec<ClassName>,
    nonconformant: Vec<ClassName>,
    selects: BTreeMap<ClassName, Box<[FeatureName]>>,
    redefines: BTreeMap<ClassName, Box<[FeatureName]>>,
    undefines: BTreeMap<ClassName, Box<[FeatureName]>>,
    renames: BTreeMap<ClassName, BTreeMap<FeatureName, FeatureName>>,
}

impl Parents {
    pub fn add_conformant<T: Into<ClassName>>(&mut self, class_name: T) {
        self.conformant.push(class_name.into())
    }

    pub fn add_rename<N, R, S, T>(&mut self, parent_name: N, rename: R)
    where
        N: Into<ClassName>,
        R: IntoIterator<Item = (S, T)>,
        S: Into<FeatureName>,
        T: Into<FeatureName>,
    {
        let class_name = parent_name.into();
        let rename = rename
            .into_iter()
            .map(|(source, target)| (source.into(), target.into()));
        match self.renames.get_mut(&class_name) {
            Some(renames) => renames.extend(rename),
            None => {
                let renames = BTreeMap::from_iter(rename);
                self.renames.insert(class_name.into(), renames);
            }
        }
    }

    pub fn add_redefines<N, F>(&mut self, parent_name: N, features_names_to_redefine: F)
    where
        N: Into<ClassName>,
        F: IntoIterator<Item = FeatureName>,
    {
        let parent_name = parent_name.into();
        let _ = self.redefines.insert(
            parent_name,
            features_names_to_redefine.into_iter().collect(),
        );
    }

    pub fn names_conformant(&self) -> &Vec<ClassName> {
        &self.conformant
    }

    pub fn names_nonconformant(&self) -> &Vec<ClassName> {
        &self.nonconformant
    }

    fn names(&self) -> impl Iterator<Item = &ClassName> {
        self.conformant.iter().chain(self.nonconformant.iter())
    }

    fn simple_iter(&self) -> impl Iterator<Item = (&ClassName, &Self)> {
        self.names().map(move |name| (name, self))
    }

    pub fn selects(&self, parent_name: &ClassName) -> Option<&[FeatureName]> {
        self.selects.get(parent_name).map(|v| v.as_ref())
    }

    pub fn redefines(&self, parent_name: &ClassName) -> Option<&[FeatureName]> {
        self.redefines.get(parent_name).map(|v| v.as_ref())
    }

    pub fn undefines(&self, parent_name: &ClassName) -> Option<&[FeatureName]> {
        self.undefines.get(parent_name).map(|v| v.as_ref())
    }

    pub fn rename_maps(
        &self,
        parent_id: &ClassName,
    ) -> Option<&BTreeMap<FeatureName, FeatureName>> {
        self.renames.get(parent_id)
    }
}

#[cfg(test)]
impl Parents {
    fn mock_parents() -> Self {
        let mut parents = Self::default();
        parents.add_conformant("PARENT");
        parents.add_rename("PARENT", [("parent_feature", "child_feature")]);
        parents
    }
    fn mock_grandparents() -> Self {
        let mut parents = Self::default();
        parents.add_conformant("GRANDPARENT");
        parents.add_rename("GRANDPARENT", [("grandparent_feature", "parent_feature")]);
        parents
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Default)]
pub struct Classes {
    to_names: HashMap<ClassID, ClassName>,
    from_names: HashMap<ClassName, ClassID>,
    locations: HashMap<ClassID, Location>,
    features: HashMap<ClassID, usize>,
    models: HashMap<ClassID, Model>,
    ranges: HashMap<ClassID, Range>,
    parents: HashMap<ClassID, Parents>,
}

impl<I, N, T> From<T> for Classes
where
    I: Into<ClassID>,
    N: Into<ClassName>,
    T: IntoIterator<Item = (I, N)> + Clone,
{
    fn from(value: T) -> Self {
        Self {
            to_names: value
                .clone()
                .into_iter()
                .map(|(id, name)| (id.into(), name.into()))
                .collect(),
            from_names: value
                .into_iter()
                .map(|(id, name)| (name.into(), id.into()))
                .collect(),
            ..Default::default()
        }
    }
}

impl Classes {
    fn add_features(&mut self, class_id: ClassID, features_number: usize) {
        self.features.insert(class_id, features_number);
    }

    fn add_parenting<T: Into<Parents>>(&mut self, class_id: ClassID, parents: T) {
        let parents = parents.into();
        self.parents.insert(class_id, parents);
    }
}

impl Classes {
    pub fn name(&self, id: ClassID) -> &ClassName {
        self.to_names
            .get(&id)
            .unwrap_or_else(|| panic!("fails to find name for class with id: {id:#?}"))
    }

    pub fn id<T: AsRef<str> + std::fmt::Debug>(&self, name: T) -> ClassID {
        *self
            .from_names
            .get(name.as_ref())
            .unwrap_or_else(|| panic!("fails to find id for class named: {name:#?}"))
    }

    pub fn parents(&self, id: ClassID) -> Option<&Parents> {
        self.parents.get(&id)
    }

    fn iter_parents(&self, id: ClassID) -> impl Iterator<Item = (&ClassName, &Parents)> {
        self.parents
            .get(&id)
            .into_iter()
            .flat_map(|parents| parents.simple_iter())
    }

    pub fn features(&self, id: ClassID) -> impl Iterator<Item = FeatureID> + use<'_> {
        let ClassID(class_id_val) = id;
        self.features
            .get(&id)
            .into_iter()
            .flat_map(|&feature_number| (0 as usize)..feature_number)
            .map(move |ft_count| (class_id_val, ft_count).into())
    }

    pub fn model(&self, id: ClassID) -> Option<&Model> {
        self.models.get(&id)
    }
}

pub fn all_features(
    base_classes: &Classes,
    base_features: &Features,
    class_id: ClassID,
) -> Vec<FeatureID> {
    base_classes
        .iter_parents(class_id)
        .into_iter()
        .flat_map(|(parent_name, _)| {
            all_features(base_classes, base_features, base_classes.id(parent_name))
        })
        .chain(base_classes.features(class_id))
        .collect()
}

fn feature_renames<'cls: 'fts, 'fts>(
    base_classes: &'cls Classes,
    base_features: &'fts Features,
    id: ClassID,
) -> HashMap<FeatureID, &'fts FeatureName> {
    base_classes
        .iter_parents(id)
        .filter_map(|(name, properties)| properties.renames.get(name).map(|rename| (name, rename)))
        .map(|(name, name_map)| {
            let id = base_classes.id(name);

            let mut partial_renames = feature_renames(&base_classes, &base_features, id);

            let local_renames: Vec<_> = all_features(&base_classes, &base_features, id)
                .into_iter()
                .filter_map(|ft_id| {
                    let old_name = partial_renames
                        .get(&ft_id)
                        .copied()
                        .unwrap_or_else(|| base_features.name(ft_id));

                    name_map.get(&old_name).map(|new_name| (ft_id, new_name))
                })
                .collect();

            partial_renames.extend(local_renames);
            partial_renames
        })
        .reduce(|mut acc, renames| {
            acc.extend(renames);
            acc
        })
        .unwrap_or_default()
}

pub fn feature_name<'cls: 'fts, 'fts>(
    base_classes: &'cls Classes,
    base_features: &'fts Features,
    class_id: ClassID,
    feature_id: FeatureID,
) -> &'fts FeatureName {
    let renames = feature_renames(base_classes, base_features, class_id);
    renames
        .get(&feature_id)
        .copied()
        .unwrap_or_else(|| new_feature::base_name(base_features, feature_id))
}

#[cfg(test)]
impl Classes {
    /// Classes containing:
    /// {ClassID(0 as usize) ClassName("CHILD")}
    /// {ClassID(1 as usize) ClassName("PARENT")}
    /// {ClassID(2 as usize) ClassName("GRANDPARENT")}
    pub fn mock_inheritance() -> Self {
        let mut cls = Classes::from([(0 as usize, "CHILD"), (1, "PARENT"), (2, "GRANDPARENT")]);
        let child_id = cls.id("CHILD");
        let parent_id = cls.id("PARENT");
        let grandparent_id = cls.id("GRANDPARENT");

        cls.add_parenting(child_id, Parents::mock_parents());
        cls.add_parenting(parent_id, Parents::mock_grandparents());
        cls.add_features(grandparent_id, 1);
        cls
    }

    /// Classes containing:
    /// {ClassID(0 as usize) ClassName("TEST") FeatureNumber(1)}
    pub fn mock_singleton() -> Self {
        let mut cls = Classes::from([(0 as usize, "TEST")]);
        let id = cls.id("TEST");
        cls.add_features(id, 1);
        cls
    }
}

trait InheritorCompliant {
    fn classes(&self) -> &Classes;
    fn features(&self) -> &Features;
}

trait Inheritor: InheritorCompliant {
    fn name_mapping(&self, id: ClassID) -> HashMap<FeatureID, &FeatureName>;
    fn immediate_and_inherited_features<T>(&self, id: ClassID) -> T
    where
        T: FromIterator<FeatureID>;
}

impl<V: InheritorCompliant> Inheritor for V {
    fn name_mapping(&self, id: ClassID) -> HashMap<FeatureID, &FeatureName> {
        self.classes()
            .iter_parents(id)
            .filter_map(|(name, properties)| {
                properties.renames.get(name).map(|rename| (name, rename))
            })
            .map(|(name, name_map)| {
                let id = self.classes().id(name);

                let mut partial_renames = self.name_mapping(id);

                let local_renames: Vec<_> = self
                    .immediate_and_inherited_features::<Vec<_>>(id)
                    .into_iter()
                    .filter_map(|id| {
                        let old_name = partial_renames
                            .get(&id)
                            .map_or_else(|| self.features().name(id), |&ft| ft);

                        name_map.get(old_name).map(|new_name| (id, new_name))
                    })
                    .collect();

                partial_renames.extend(local_renames);
                partial_renames
            })
            .reduce(|mut acc, renames| {
                acc.extend(renames);
                acc
            })
            .unwrap_or_default()
    }

    fn immediate_and_inherited_features<T>(&self, id: ClassID) -> T
    where
        T: FromIterator<FeatureID>,
    {
        self.classes()
            .iter_parents(id)
            .into_iter()
            .flat_map(|(parent_name, _)| {
                self.immediate_and_inherited_features::<Vec<_>>(self.classes().id(parent_name))
            })
            .chain(self.classes().features(id))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::code_entities::new_class;

    #[test]
    fn transitive_renaming_of_inherited_features() {
        let classes = Classes::mock_inheritance();
        let features = Features::mock_inheritance();

        let class_child = classes.id(&ClassName("CHILD".to_string()));

        eprintln!("class_child: {:#?}", class_child);
        eprintln!("classes: {:#?}", classes);
        eprintln!("features: {:#?}", features);

        let current_features = new_class::all_features(&classes, &features, class_child);

        eprintln!("features' ids: {:#?}", features);

        let features_names: Vec<_> = current_features
            .into_iter()
            .map(|ft_id| new_class::feature_name(&classes, &features, class_child, ft_id))
            .collect();

        assert!(
            features_names
                .iter()
                .any(|name| name.as_str() == "child_feature"),
            "fails to find immediate feature: child_feature\nIn feature names: {:#?}",
            features_names,
        );
    }
}
