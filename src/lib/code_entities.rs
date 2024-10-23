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
        Contract, ContractClause, ContractKeyword, Postcondition, Precondition, Predicate, Tag,
    };
    pub(crate) use super::feature::Feature;
    pub(crate) use super::shared::{FindDefinition, Location, Point, Range};
    pub(crate) use super::CodeEntity;
}
pub(crate) trait CodeEntity {}

#[cfg(test)]
mod tests {}
