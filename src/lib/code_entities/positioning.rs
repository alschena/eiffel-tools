use crate::lib::code_entities::class::ClassID;
use crate::lib::code_entities::feature::FeatureID;
use std::collections::HashMap;

mod utils;

use utils::Location;
use utils::Point;
use utils::Range;

struct ClassLocation(HashMap<ClassID, Location>);
struct FeatureLocation(HashMap<FeatureID, Location>);

struct ClassRange(HashMap<ClassID, Range>);
struct FeatureRange(HashMap<FeatureID, Range>);
