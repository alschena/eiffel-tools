use crate::lib::code_entities::prelude::*;
use crate::lib::config::System;
use crate::lib::parser::Parser;
use crate::lib::parser::Tree;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tracing::instrument;
use tracing::warn;

#[derive(Debug, Default)]
pub struct Workspace {
    location_tree: HashMap<PathBuf, Tree>,
    location_class: HashMap<PathBuf, ClassName>,
    class_location: HashMap<ClassName, PathBuf>,
    classes: Vec<Class>,
}

impl Workspace {
    pub(crate) fn new() -> Workspace {
        Workspace::default()
    }

    pub fn add_file(&mut self, value: (Class, PathBuf, Tree)) {
        let (class, pathbuf, tree) = value;
        let classname = class.name().clone();

        self.location_tree.insert(pathbuf.clone(), tree);

        self.location_class
            .insert(pathbuf.clone(), classname.clone());

        self.class_location.insert(classname, pathbuf);

        self.classes.push(class);
    }

    pub fn class(&self, path: &Path) -> Option<&Class> {
        self.location_class.get(path).map(|name| {
            self.classes
                .iter()
                .find(|class| class.name() == name)
                .unwrap_or_else(|| unreachable!("fails to find class with name {:#?}", name))
        })
    }

    pub fn path(&self, classname: &ClassName) -> &Path {
        self.class_location
            .get(classname)
            .unwrap_or_else(|| unreachable!("fails to find location of class {:#?}", classname))
    }

    pub fn feature_around(&self, path: &Path, point: Point) -> Option<&Feature> {
        match self.class(path) {
            Some(class) => Feature::feature_around_point(class.features().iter(), point),
            None => {
                warn!("fails to find classname at {:#?}", path);
                return None;
            }
        }
    }

    async fn read_file(path: &Path) -> Option<String> {
        tokio::fs::read(path)
            .await
            .inspect_err(|e| warn!("fails to read file at path: {:#?} with error: {:#?}", path, e))
            .into_iter()
            .flat_map(|byte_source| {
                String::from_utf8(byte_source)
                    .inspect_err(|e| warn!("fails to convert file at path: {:#?} from byte form to utf-8 with error: {:#?}",path, e))
            }).next()
    }

    pub async fn reload(&mut self, pathbuf: PathBuf) {
        match self.location_class.get(&pathbuf) {
            Some(class_name) => {
                self.classes.retain(|cl| cl.name() != class_name);
            }
            None => {
                warn!("calls reload on new class.");
            }
        }

        let src = Self::read_file(&pathbuf).await;

        if let Some(source) = src {
            let mut parser = Parser::new();
            match parser.processed_file(source) {
                Ok((class, tree)) => {
                    self.add_file((class, pathbuf, tree));
                }
                Err(e) => {
                    warn!(
                        "fails to reload the file at {:#?} in the workspace file with error: {:#?}",
                        pathbuf, e
                    )
                }
            }
        }
    }

    pub fn system_classes(&self) -> &Vec<Class> {
        &self.classes
    }

    #[instrument(skip_all)]
    pub async fn load_system(&mut self, system: &System) {
        let eiffel_files = system.eiffel_files();

        let (transmitter, mut receiver) = tokio::sync::mpsc::unbounded_channel();

        tokio::spawn(async move {
            for filepath in eiffel_files {
                let mut parser = Parser::new();
                let transmitter = transmitter.clone();

                tokio::spawn(async move {
                    if let Some(source) = Self::read_file(&filepath).await {
                        tokio_rayon::spawn(move || match parser.processed_file(source) {
                            Ok((class, tree)) => {
                                transmitter
                                    .send((class, filepath, tree))
                                    .inspect_err(|e| {
                                        warn!("fails to send parsed file with error: {:#?}", e)
                                    })
                                    .ok();
                            }
                            Err(e) => {
                                warn!("fails to parse file with error {:#?}", e);
                            }
                        })
                        .await
                    }
                });
            }
        });

        while let Some(value) = receiver.recv().await {
            self.add_file(value);
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;
    use crate::lib::parser::Parser;
    use assert_fs::prelude::*;
    use assert_fs::{fixture::FileWriteStr, TempDir};

    impl Workspace {
        pub fn mock() -> Self {
            Self::default()
        }
        pub fn is_mock(&self) -> bool {
            self.classes.is_empty()
        }
    }

    #[tokio::test]
    async fn reload() {
        let mut parser = Parser::new();
        let temp_dir = TempDir::new().expect("must create temporary directory.");
        let file = temp_dir.child("processed_file_new.e");
        let source = r#"
class A
feature
  x: INTEGER
end
            "#;
        file.write_str(source).expect("write to file");
        assert!(file.exists());

        let mut ws = Workspace::mock();

        let (cl, tr) = parser
            .processed_file(source)
            .expect("fails to process tmp file");

        ws.add_file((cl, file.to_path_buf(), tr));

        let class_a_is_in_workspace = ws
            .location_class
            .get(&file.to_path_buf())
            .is_some_and(|name| name == "A");

        assert!(class_a_is_in_workspace);

        let class_a_has_one_feature = ws
            .classes
            .first()
            .is_some_and(|class| class.features().len() == 1);

        assert!(class_a_has_one_feature);

        file.write_str(
            r#"
class A
feature
  x: INTEGER
  y: INTEGER
end
            "#,
        )
        .expect("temp file must be writable");

        ws.reload(file.to_path_buf()).await;

        assert_eq!(
            ws.classes.len(),
            1,
            "Reloading does not change the number of classes in the system"
        );
        assert_eq!(
            ws.classes[0].features().len(),
            2,
            "Reloaded class has two features"
        );
    }
}
