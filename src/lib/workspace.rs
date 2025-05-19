use crate::lib::code_entities::prelude::*;
use crate::lib::config::System;
use crate::lib::parser::Parser;
use crate::lib::parser::Tree;
use crate::lib::processed_file::ProcessedFile;
use rayon::iter::ParallelDrainRange;
use std::collections::HashMap;
use std::path::Path;
use std::path::PathBuf;
use tokio::task::JoinSet;
use tracing::warn;

#[derive(Debug, Default)]
pub struct Workspace {
    pub path_to_tree: HashMap<PathBuf, Tree>,
    pub path_to_classname: HashMap<PathBuf, ClassName>,
    pub classes: Vec<Class>,
}

impl Workspace {
    pub(crate) fn new() -> Workspace {
        Workspace::default()
    }

    pub fn add_file(&mut self, value: (Class, PathBuf, Tree)) {
        let (class, pathbuf, tree) = value;
        let classname = class.name().clone();

        self.path_to_tree.insert(pathbuf.clone(), tree);
        self.path_to_classname.insert(pathbuf, classname);
        self.classes.push(class);
    }

    pub fn feature_around(&self, path: &Path, point: Point) -> Option<&Feature> {
        let Some(classname) = self.path_to_classname.get(path) else {
            warn!("fails to find classname at {:#?}", path);
            return None;
        };

        let Some(class) = self.classes.iter().find(|class| classname == class.name()) else {
            unreachable!("fails to find class with name {:#?}", classname)
        };

        Feature::feature_around_point(class.features().iter(), point)
    }

    pub async fn reload(&mut self, pathbuf: PathBuf) {
        match self.path_to_classname.get(&pathbuf) {
            Some(class_name) => {
                self.classes.retain(|cl| cl.name() != class_name);
            }
            None => {
                warn!("calls reload on new class.");
            }
        }

        let mut parser = Parser::new();
        match parser.processed_file(pathbuf).await {
            Ok(val) => {
                self.add_file(val);
            }
            Err(e) => {
                warn!("fails to add a file to the workspace file with error: {e:#?}")
            }
        }
    }

    pub fn system_classes(&self) -> &Vec<Class> {
        &self.classes
    }

    pub async fn load_system(&mut self, system: &System) {
        let eiffel_files = system.eiffel_files();
        let (sender, mut receiver) = tokio::sync::mpsc::channel(100);

        {
            let sender = sender;
            for filepath in eiffel_files {
                let mut parser = Parser::new();
                let sender = sender.clone();
                tokio::spawn(async move {
                    match parser.processed_file(filepath).await {
                        Ok(value) => {
                            let status = sender.send(value).await;
                            if let Err(e) = status {
                                warn!(
                                    "fails to send processed file through mpsc with error: {e:#?}"
                                )
                            }
                        }
                        Err(e) => {
                            warn!("fails to parse file with error {e}")
                        }
                    }
                });
            }
        }

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
        file.write_str(
            r#"
class A
feature
  x: INTEGER
end
            "#,
        )
        .expect("write to file");
        assert!(file.exists());

        let mut ws = Workspace::mock();

        let val = parser
            .processed_file(file.to_path_buf())
            .await
            .expect("fails to process tmp file");

        ws.add_file(val);

        let class_a_is_in_workspace = ws
            .path_to_classname
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
