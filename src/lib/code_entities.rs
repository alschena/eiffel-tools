use super::processed_file::ProcessedFile;

mod class;
pub(crate) mod contract;
mod feature;
mod shared;
pub(crate) mod prelude {
    pub(crate) use super::class::Class;
    pub(crate) use super::contract;
    pub(crate) use super::feature::Feature;
    pub(crate) use super::shared::{Location, Point, Range};
    pub(crate) use super::{CodeEntity, Indent};
}
pub(crate) trait CodeEntity {}
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

#[cfg(test)]
mod tests {}
