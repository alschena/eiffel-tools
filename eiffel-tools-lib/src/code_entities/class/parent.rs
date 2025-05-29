use crate::code_entities::prelude::Class;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Parent {
    pub name: String,
    pub select: Vec<String>,
    pub rename: Vec<(String, String)>,
    pub redefine: Vec<String>,
    pub undefine: Vec<String>,
}
impl Parent {
    pub fn class<'a>(&self, system_classes: &'a [Class]) -> Option<&'a Class> {
        system_classes
            .into_iter()
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

impl Default for Parent {
    fn default() -> Self {
        Self {
            name: String::new(),
            select: Vec::new(),
            rename: Vec::new(),
            redefine: Vec::new(),
            undefine: Vec::new(),
        }
    }
}
