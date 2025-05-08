use crate::lib::code_entities::new_class::ClassID;
use crate::lib::code_entities::new_feature::FeatureID;
use std::collections::BTreeMap;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub(super) struct Parents {
    pub(super) conformant: Vec<ClassID>,
    pub(super) nonconformant: Vec<ClassID>,
    pub(super) selects: Vec<(ClassID, FeatureID)>,
    pub(super) redefines: Vec<(ClassID, FeatureID)>,
    pub(super) undefines: Vec<(ClassID, FeatureID)>,
    pub(super) renames: BTreeMap<ClassID, (FeatureID, String)>,
}

impl Extend<Parents> for Parents {
    fn extend<T: IntoIterator<Item = Parents>>(&mut self, iter: T) {
        let Self {
            conformant: current_conformant,
            nonconformant: current_nonconformant,
            selects: current_selects,
            redefines: current_redefines,
            undefines: current_undefines,
            renames: current_renames,
        } = self;

        for Self {
            mut conformant,
            mut nonconformant,
            mut selects,
            mut redefines,
            mut undefines,
            mut renames,
        } in iter
        {
            current_conformant.append(conformant.as_mut());
            current_nonconformant.append(nonconformant.as_mut());
            current_selects.append(selects.as_mut());
            current_redefines.append(redefines.as_mut());
            current_undefines.append(undefines.as_mut());
            current_renames.append(&mut renames);
        }
    }
}

impl Parents {
    // pub fn rename<'slf, T: Iterator<Item = &'slf FeatureID>>(
    //     &'slf self,
    //     class: &'slf ClassID,
    //     features_old_names: T,
    // ) -> impl Iterator<Item = &'slf String> + use<'slf, T> {
    //     let mapping = self
    //         .renames
    //         .iter()
    //         .filter_map(|(id, old_name, new_name)| (id == class).then_some((old_name, new_name)))
    //         .collect::<HashMap<_, _>>();
    //     features_old_names
    //         .filter_map(|old_name| mapping.get(old_name))
    //         .copied()
    // }
}
