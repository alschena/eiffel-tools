mod class;
pub(crate) mod contract;
mod feature;
mod shared;
pub(crate) mod prelude {
    pub(crate) use super::class::Class;
    pub(crate) use super::class::Model as ClassModel;
    pub(crate) use super::contract;
    pub(crate) use super::feature::Feature;
    pub(crate) use super::shared::{Location, Point, Range};
    pub(crate) use super::{Indent, ValidSyntax};
}
pub(crate) trait Indent {
    const INDENTATION_LEVEL: u32;
    const INDENTATION_CHARACTER: char = '\t';
    fn indentation_string() -> String {
        (0..Self::INDENTATION_LEVEL)
            .into_iter()
            .fold(String::new(), |mut acc, _| {
                acc.push(Self::INDENTATION_CHARACTER);
                acc
            })
    }
}
pub(crate) trait ValidSyntax {
    fn valid_syntax(&self) -> bool;
}

#[cfg(test)]
mod tests {}
