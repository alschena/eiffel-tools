mod class;
pub(crate) mod contract;
mod feature;
mod shared;
pub(crate) mod prelude {
    pub(crate) use super::class::model::ModelExtended as ClassModel;
    pub(crate) use super::class::{Class, ClassName};
    pub(crate) use super::contract;
    pub(crate) use super::feature::EiffelType;
    pub(crate) use super::feature::Feature;
    pub(crate) use super::feature::FeatureVisibility;
    pub(crate) use super::feature::Notes as FeatureNotes;
    pub(crate) use super::feature::Parameters as FeatureParameters;
    pub(crate) use super::shared::{Location, Point, Range};
    pub(crate) use super::Indent;
}
pub(crate) trait Indent {
    const INDENTATION_LEVEL: usize;
    const INDENTATION_CHARACTER: char = '\t';
    fn indentation_string() -> String {
        (0..Self::INDENTATION_LEVEL).into_iter().fold(
            String::with_capacity(Self::INDENTATION_LEVEL),
            |mut acc, _| {
                acc.push(Self::INDENTATION_CHARACTER);
                acc
            },
        )
    }
}

#[cfg(test)]
mod tests {}
