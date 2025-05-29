mod class;
pub(crate) mod contract;
mod feature;
mod shared;

pub(crate) mod prelude {
    pub(crate) use super::class::model::Model as ClassLocalModel;
    pub(crate) use super::class::model::ModelExtended as ClassModel;
    pub(crate) use super::class::model::ModelNames;
    pub(crate) use super::class::Class;
    pub(crate) use super::class::ClassID;
    pub(crate) use super::class::ClassName;
    pub(crate) use super::class::Parent as ClassParent;
    pub(crate) use super::contract;
    pub(crate) use super::feature::EiffelType;
    pub(crate) use super::feature::Feature;
    pub(crate) use super::feature::FeatureVisibility;
    pub(crate) use super::feature::Notes as FeatureNotes;
    pub(crate) use super::feature::Parameters as FeatureParameters;
    pub(crate) use super::shared::{Location, Point, Range};
}
