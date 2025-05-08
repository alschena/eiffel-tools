use crate::lib::code_entities::new_class::*;
use crate::lib::code_entities::new_feature::*;
use crate::lib::code_entities::prelude::Location;
use crate::lib::parser::Tree;

pub struct Workspace(Vec<(Tree, Location)>, Classes);
