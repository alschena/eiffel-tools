use super::prelude::*;
use async_lsp::lsp_types;
use std::borrow::Cow;
use std::collections::HashSet;
use std::fmt::Display;

pub mod model;
use model::*;

pub mod parent;
pub use parent::Parent;

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct ClassName(pub String);

impl Display for ClassName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl ClassName {
    pub fn is_terminal_for_model(&self) -> bool {
        let ClassName(name) = self;
        match name.as_str() {
            "BOOLEAN" => true,
            "INTEGER" => true,
            "REAL" => true,
            "MML_SEQUENCE" => true,
            "MML_BAG" => true,
            "MML_SET" => true,
            "MML_MAP" => true,
            "MML_PAIR" => true,
            "MML_RELATION" => true,
            _ => false,
        }
    }

    pub fn inhereted_model<'system, 'class_name>(
        &'class_name self,
        system_classes: &'system [Class],
    ) -> Option<Model> {
        if self.is_terminal_for_model() {
            return None;
        };

        let model = system_classes
            .iter()
            .find(|c| c.name() == self)
            .map(|class| class.model_with_inheritance(system_classes))
            .unwrap_or_default();

        Some(model)
    }

    pub fn model_extended<'class_name, 'system: 'class_name>(
        &'class_name self,
        system_classes: &'system [Class],
    ) -> ModelExtended {
        if self.is_terminal_for_model() {
            return ModelExtended::Terminal;
        }
        system_classes
            .iter()
            .find(|c| c.name() == self)
            .map(|class| class.model_extended(system_classes))
            .unwrap_or_default()
    }
}

impl PartialEq<str> for ClassName {
    fn eq(&self, other: &str) -> bool {
        matches!(self, ClassName(name) if name == other)
    }
}

impl<T: AsRef<str>> PartialEq<T> for ClassName {
    fn eq(&self, other: &T) -> bool {
        matches!(self, ClassName(name) if name == other.as_ref())
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct ClassID(ClassName, Location);

#[derive(Debug, PartialEq, Eq, Clone, Hash, Default)]
pub struct Class {
    pub name: ClassName,
    pub model: Model,
    pub features: Vec<Feature>,
    pub parents: Vec<Parent>,
    pub range: Range,
}

impl Class {
    pub fn name(&self) -> &ClassName {
        &self.name
    }

    fn local_model(&self) -> &Model {
        &self.model
    }

    fn model_with_inheritance<'a>(&'a self, system_classes: &'a [Class]) -> Model {
        let mut model = self.local_model().clone();
        for mut ancestor_model in self
            .ancestor_classes(system_classes)
            .into_iter()
            .map(|parents| parents.local_model())
            .cloned()
        {
            model.append(&mut ancestor_model);
        }
        model
    }

    fn model_extended<'class, 'system: 'class>(
        &'class self,
        system_classes: &'system [Class],
    ) -> ModelExtended {
        self.model_with_inheritance(system_classes)
            .extended(system_classes)
    }

    pub fn features(&self) -> &Vec<Feature> {
        &self.features
    }

    pub fn into_features(self) -> Vec<Feature> {
        self.features
    }

    fn parents(&self) -> &Vec<Parent> {
        &self.parents
    }

    fn parent_classes<'a>(
        &'a self,
        system_classes: &'a [Class],
    ) -> impl Iterator<Item = &'a Class> {
        self.parents()
            .into_iter()
            .filter_map(|parent| parent.class(system_classes))
    }

    pub fn ancestors<'a>(&'a self, system_classes: &'a [Class]) -> HashSet<&'a Parent> {
        let mut ancestors = HashSet::new();
        for parent in self.parents() {
            let Some(parent_class) = parent.class(system_classes) else {
                continue;
            };
            ancestors.insert(parent);
            ancestors.extend(parent_class.ancestors(system_classes));
        }
        ancestors
    }

    pub fn ancestor_classes<'a>(&'a self, system_classes: &'a [Class]) -> HashSet<&'a Class> {
        let mut ancestors_classes = HashSet::new();
        self.parent_classes(system_classes)
            .for_each(|parent_class| {
                ancestors_classes.insert(parent_class);
                ancestors_classes.extend(parent_class.ancestor_classes(system_classes));
            });
        ancestors_classes
    }

    fn inhereted_features<'a>(&'a self, system_classes: &'a [Class]) -> Vec<Cow<'a, Feature>> {
        self.parent_classes(system_classes)
            .into_iter()
            .zip(self.parents())
            .flat_map(|(parent_class, parent)| {
                parent_class
                    .inhereted_features(system_classes)
                    .into_iter()
                    .chain(parent_class.features().iter().map(|f| Cow::Borrowed(f)))
                    .map(|feature| {
                        match parent
                            .rename
                            .iter()
                            .find(|(old_name, _)| old_name == feature.name())
                        {
                            Some((_, new_name)) => {
                                Cow::Owned(feature.clone_rename(new_name.to_string()))
                            }
                            None => feature,
                        }
                    })
            })
            .collect()
    }

    pub fn immediate_and_inherited_features<'slf>(
        &'slf self,
        system_classes: &'slf [Class],
    ) -> Vec<Cow<'slf, Feature>> {
        self.features()
            .into_iter()
            .map(|feature| Cow::Borrowed(feature))
            .chain(self.inhereted_features(system_classes))
            .collect::<Vec<_>>()
    }

    pub fn range(&self) -> &Range {
        &self.range
    }

    pub fn from_name_range(name: ClassName, range: Range) -> Class {
        Class {
            name,
            model: Model::default(),
            features: Vec::new(),
            parents: Vec::new(),
            range,
        }
    }

    pub fn add_feature(&mut self, feature: &Feature) {
        self.features.push(feature.clone())
    }

    pub fn add_model(&mut self, model: &Model) {
        self.model = model.clone()
    }

    #[cfg(test)]
    pub fn add_parent(&mut self, parent: Parent) {
        self.parents.push(parent)
    }
}

impl TryFrom<&Class> for lsp_types::DocumentSymbol {
    type Error = anyhow::Error;

    fn try_from(value: &Class) -> std::result::Result<Self, Self::Error> {
        let ClassName(name) = value.name().to_owned();
        let features = value.features();
        let range = value.range().clone().try_into()?;
        let children: Option<Vec<lsp_types::DocumentSymbol>> = Some(
            features
                .into_iter()
                .map(|x| x.try_into().expect("feature conversion to document symbol"))
                .collect(),
        );
        Ok(lsp_types::DocumentSymbol {
            name,
            detail: None,
            kind: lsp_types::SymbolKind::CLASS,
            tags: None,
            deprecated: None,
            range,
            selection_range: range,
            children,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lib::parser::Parser;
    use anyhow::Result;
    use std::fs::File;
    use std::io::prelude::*;
    use std::path::PathBuf;

    fn class(source: &str) -> anyhow::Result<Class> {
        let mut parser = Parser::new();
        parser.class_from_source(source)
    }

    #[test]
    fn parse_annotated_class() -> anyhow::Result<()> {
        let src = "
note
  demo_note: True
  multi_note: True, False
class DEMO_CLASS
invariant
  note
    note_after_invariant: True
end
    ";
        let class = class(src)?;

        assert_eq!(class.name(), "DEMO_CLASS");
        Ok(())
    }

    #[test]
    fn rename_inherit_features() -> anyhow::Result<()> {
        let child_src = "
class A
inherit
  B
    rename y as z
end
";
        let parent_src = "
class B
inherit
  C
    rename x as y
end
";
        let grandparent_src = "
class C
feature
    x: BOOLEAN
end
";
        let grandparent = class(&grandparent_src)?;
        let parent = class(&parent_src)?;
        let child = class(&child_src)?;

        let system_classes = vec![grandparent.clone(), parent.clone(), child.clone()];
        let child_features = child.inhereted_features(&system_classes);
        let parent_features = parent.inhereted_features(&system_classes);
        assert_eq!(
            grandparent.features().first().unwrap().name(),
            "x",
            "grandparent features"
        );
        assert_eq!(
            parent_features.first().unwrap().name(),
            "y",
            "parent features"
        );
        assert_eq!(
            child_features.first().unwrap().name(),
            "z",
            "child features"
        );
        Ok(())
    }

    #[test]
    fn parse_parent_classes() -> anyhow::Result<()> {
        let src_child = "
    class A
    inherit {NONE}
      X Y Z

    inherit
      W
        undefine a
        redefine c
        rename e as f
        export
          {ANY}
            -- Header comment
            all
        select g
        end
    end
    ";
        let src_parent = "
    note
        model: seq
    class W
    feature
        x: INTEGER
        seq: MML_SEQUENCE [INTEGER]
    end
    ";
        let system_classes = vec![class(src_child)?, class(src_parent)?];
        let child = &system_classes[0];
        let parent = &system_classes[1];
        let child_parents = child.parent_classes(&system_classes).collect::<Vec<_>>();
        assert_eq!(child_parents, vec![parent]);
        Ok(())
    }

    #[test]
    fn ancestor_classes() -> anyhow::Result<()> {
        let child_src = "
class A
inherit
  B
    rename y as z
end
";
        let parent_src = "
class B
inherit
  C
    rename x as y
end
";
        let grandparent_src = "
class C
feature
    x: BOOLEAN
end
";
        let system_classes = vec![
            class(child_src)?,
            class(parent_src)?,
            class(grandparent_src)?,
        ];
        let child = &system_classes[0];
        let parent = &system_classes[1];
        let grandparent = &system_classes[2];

        let mut child_ancestors = HashSet::new();
        child_ancestors.insert(parent);
        child_ancestors.insert(grandparent);
        assert_eq!(child.ancestor_classes(&system_classes), child_ancestors);

        let mut parent_ancestors = HashSet::new();
        parent_ancestors.insert(grandparent);
        assert_eq!(parent.ancestor_classes(&system_classes), parent_ancestors,);
        Ok(())
    }
    #[test]
    fn full_model() -> anyhow::Result<()> {
        let src_child = "
    note
        model: seq_child
    class A
    inherit
      W
    feature seq_child: MML_SEQUENCE[G]
    end
    ";
        let src_parent = "
    note
        model: seq_parent
    class W
    feature
        x: INTEGER
        seq_parent: MML_SEQUENCE [INTEGER]
    end
    ";
        let system_classes = vec![class(src_child)?, class(src_parent)?];
        let child = &system_classes[0];
        let parent = &system_classes[1];

        let mut appended_models = child.local_model().clone();
        appended_models.append(&mut parent.local_model().clone());

        assert_eq!(
            child.model_with_inheritance(&system_classes),
            appended_models
        );
        Ok(())
    }
}
