use std::ops::Deref;

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub struct Notes(pub Vec<(String, Vec<String>)>);
impl Deref for Notes {
    type Target = Vec<(String, Vec<String>)>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
