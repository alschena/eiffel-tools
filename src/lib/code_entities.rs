use super::processed_file::ProcessedFile;

mod call;
mod class;
mod contract;
mod feature;
mod shared;
pub(crate) mod prelude {
    pub(crate) use super::call::UnqualifiedCall;
    pub(crate) use super::class::Class;
    pub(crate) use super::contract::{
        ContractBlock, ContractClause, ContractKeyword, Postcondition, Precondition, Predicate, Tag,
    };
    pub(crate) use super::feature::Feature;
    pub(crate) use super::shared::{FindDefinition, Location, Point, Range};
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
