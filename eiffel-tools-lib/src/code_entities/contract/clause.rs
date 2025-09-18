use schemars::JsonSchema;
use serde::Deserialize;
use std::fmt::Debug;
use std::fmt::Display;

#[derive(Deserialize, Debug, PartialEq, Eq, Clone, Hash, JsonSchema, Default)]
#[schemars(deny_unknown_fields)]
#[schemars(description = "A valid contract clause of the eiffel programming language.")]
pub struct Clause {
    pub tag: Tag,
    pub predicate: Predicate,
}

impl Display for Clause {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match (&self.tag, &self.predicate) {
            (_, predicate) if predicate.as_str().is_empty() => {
                write!(f, "")
            }
            (tag, predicate) if tag.as_str().is_empty() => {
                writeln!(f, "({})", predicate)
            }
            (tag, predicate) => {
                writeln!(f, "{}: {}", tag, predicate)
            }
        }
    }
}

impl Clause {
    pub fn new(tag: Tag, predicate: Predicate) -> Clause {
        Clause { tag, predicate }
    }
    pub fn from_line(line: &str) -> Option<Clause> {
        line.rsplit_once(": ").map(|(tag_str, predicate_str)| {
            Clause::new(
                Tag::new(tag_str.trim()),
                Predicate::new(predicate_str.trim()),
            )
        })
    }
}

#[derive(Deserialize, Clone, Debug, PartialEq, Eq, Hash, JsonSchema, Default)]
#[serde(transparent)]
#[schemars(deny_unknown_fields)]
#[schemars(description = "A valid tag clause for the Eiffel programming language.")]
pub struct Tag(String);

impl Tag {
    pub fn new<T: ToString>(text: T) -> Tag {
        Tag(text.to_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn trim_and_replace_space_with_underscore(&mut self) {
        self.0 = self.0.trim().replace(" ", "_");
    }
    pub fn update_to_lowercase(&mut self) {
        self.0 = self.0.to_lowercase();
    }
}

impl Display for Tag {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
impl From<String> for Tag {
    fn from(value: String) -> Self {
        Tag(value)
    }
}

#[derive(Hash, Deserialize, Debug, PartialEq, Eq, Clone, JsonSchema)]
#[serde(transparent)]
#[schemars(deny_unknown_fields)]
#[schemars(description = "A valid boolean expression for the Eiffel programming language.")]
pub struct Predicate(String);

impl From<&str> for Predicate {
    fn from(value: &str) -> Self {
        Predicate(value.to_string())
    }
}

impl Predicate {
    pub fn new<T: ToString>(text: T) -> Predicate {
        Predicate(text.to_string())
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Predicate {
    fn default() -> Self {
        Self(String::from("True"))
    }
}

impl Display for Predicate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}
