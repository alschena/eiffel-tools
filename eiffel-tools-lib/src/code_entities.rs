mod class;
pub mod contract;
mod feature;
mod shared;

pub mod prelude {
    pub use super::class::Class;
    pub use super::class::ClassID;
    pub use super::class::ClassName;
    pub use super::class::Parent as ClassParent;
    pub use super::class::model::Model as ClassLocalModel;
    pub use super::class::model::ModelExtended as ClassModel;
    pub use super::class::model::ModelNames;
    pub use super::contract;
    pub use super::feature::EiffelType;
    pub use super::feature::Feature;
    pub use super::feature::FeatureVisibility;
    pub use super::feature::Notes as FeatureNotes;
    pub use super::feature::Parameters as FeatureParameters;
    pub use super::shared::{Location, Point, Range};
}
