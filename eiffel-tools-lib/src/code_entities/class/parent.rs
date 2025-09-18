use crate::code_entities::prelude::Class;
use crate::code_entities::prelude::FeatureName;

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct Parent {
    pub name: String,
    pub select: Vec<FeatureName>,
    pub rename: Vec<(FeatureName, FeatureName)>,
    pub redefine: Vec<FeatureName>,
    pub undefine: Vec<FeatureName>,
}
impl Parent {
    pub fn class<'a>(&self, system_classes: &'a [Class]) -> Option<&'a Class> {
        system_classes
            .iter()
            .find(|class| class.name() == &self.name)
    }
    #[cfg(test)]
    pub fn from_name(name: String) -> Parent {
        Parent {
            name,
            select: Vec::new(),
            rename: Vec::new(),
            redefine: Vec::new(),
            undefine: Vec::new(),
        }
    }
}
