use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing::info;
use tracing::warn;
use walkdir::{self};
#[derive(Deserialize, Debug, PartialEq, Clone, Eq)]
struct Config {
    system: System,
}
#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct System {
    target: Target,
}
impl System {
    fn parse_from_file(file: &Path) -> Option<System> {
        match std::fs::read_to_string(file) {
            Ok(v) => match serde_xml_rs::from_str(v.as_str()) {
                Ok(v) => Some(v),
                Err(e) => {
                    info!("fails to parse the configuration file of the library with error {e:?}");
                    return None;
                }
            },
            Err(e) => {
                info!("fails reading from {file:?} with error {e:?}");
                return None;
            }
        }
    }
    /// All clusters defined locally or in the library.
    fn clusters(self) -> Vec<Cluster> {
        let mut clusters = self.target.cluster;

        let Some(libraries) = self.target.library else {
            return clusters;
        };

        for lib in libraries {
            let Some(path) = lib.location_path() else {
                continue;
            };
            let Some(mut system) = System::parse_from_file(&path) else {
                return clusters;
            };
            let Some(lib_dir) = lib.parent_directory() else {
                continue;
            };
            for lib_cluster in &mut system.target.cluster {
                lib_cluster.optionally_prepend_to_location(&lib_dir);
            }
            clusters.append(&mut system.target.cluster);
        }
        clusters
    }
    /// All eiffel files present in the system.
    pub fn eiffel_files(self) -> Result<Vec<PathBuf>> {
        let mut eiffel_files: Vec<PathBuf> = Vec::new();
        for cluster in self.clusters() {
            eiffel_files.append(&mut cluster.eiffel_files().context("cluster eiffel files")?);
        }
        Ok(eiffel_files)
    }
}
#[derive(Deserialize, Debug, PartialEq, Clone, Eq, Hash)]
struct Target {
    cluster: Vec<Cluster>,
    library: Option<Vec<Library>>,
}
#[derive(Deserialize, Debug, PartialEq, Clone, Eq, Hash)]
struct Cluster {
    name: String,
    location: String,
    recursive: Option<bool>,
    cluster: Option<Vec<Cluster>>,
}
impl Cluster {
    fn is_location_valid_absolute_path(&self) -> bool {
        match shellexpand::env(&self.location) {
            Ok(v) => Path::new(v.as_ref()).is_absolute(),
            Err(e) => {
                info! {"fails to expand cluster location with environmental variables with error {e:?}"};
                false
            }
        }
    }
    fn optionally_prepend_to_location(&mut self, path: &Path) {
        if !self.is_location_valid_absolute_path() {
            match path.join(Path::new(&self.location)).to_str() {
                Some(s) => self.location = s.to_string(),
                None => {
                    info!("fails to convert path to UFT-8 string")
                }
            }
        }
    }
    fn eiffel_files(&self) -> Result<Vec<PathBuf>> {
        let cluster_paths = match self.cluster {
            Some(ref clusters) => {
                let mut paths: Vec<PathBuf> = clusters
                    .iter()
                    .map(|cluster| {
                        Path::new(&self.location).join(Path::new(cluster.location.as_str()))
                    })
                    .collect();
                paths.push(PathBuf::from(self.location.clone()));
                paths
            }
            None => vec![PathBuf::from(self.location.clone())],
        }
        .iter()
        .filter_map(|unexpanded_path| {
            let unexpanded_path = match unexpanded_path.to_str() {
                Some(v) => v,
                None => return Some(Err(anyhow!("Fail to convert path to valid UFT-8"))),
            };
            match shellexpand::env(unexpanded_path) {
                Ok(path_as_string) => {
                    let p = PathBuf::from(
                        path_as_string
                            .replace(r#"/"#, std::path::MAIN_SEPARATOR_STR)
                            .replace(r#"\"#, std::path::MAIN_SEPARATOR_STR)
                            .replace(r#"$|"#, ""),
                    );
                    if p.exists() {
                        match fs::canonicalize(p) {
                            Ok(p) => Some(Ok(p)),
                            Err(e) => Some(Err(anyhow!(
                                "Path could not be canonicalized with error {:?}",
                                e
                            ))),
                        }
                    } else {
                        warn!("Tried to analyze the following inexistent path {:?}", p);
                        None
                    }
                }
                Err(e) => Some(Err(anyhow!(
                    "Fail to expand path with env variables. Error: {:?}",
                    e
                ))),
            }
        })
        .collect::<Result<Vec<PathBuf>>>()?;
        let folded_paths = cluster_paths
            .iter()
            .filter_map(|path| match self.recursive {
                Some(true) => match walkdir::WalkDir::new(path)
                    .into_iter()
                    .filter_map(|entry| match entry {
                        Ok(entry) => match entry.path().extension() {
                            Some(ext) if ext == "e" => Some(Ok(entry.path().to_owned())),
                            _ => None,
                        },
                        Err(e) => Some(Err(anyhow!(
                            "Invalid path in recursive walk with error {:?}",
                            e
                        ))),
                    })
                    .collect::<Result<Vec<PathBuf>>>()
                {
                    v @ Ok(_) => Some(v),
                    e @ Err(_) => Some(e),
                },
                _ => match fs::read_dir(path) {
                    Ok(entries) => match entries
                        .filter_map(|entry| match entry {
                            Ok(v) => match v.path().extension() {
                                Some(ext) if ext == "e" => Some(Ok(v.path().to_owned())),
                                _ => None,
                            },
                            Err(e) => Some(Err(anyhow!(
                                "Entry in recursive walk is invalid with error {:?}",
                                e
                            ))),
                        })
                        .collect::<Result<Vec<PathBuf>>>()
                    {
                        v @ Ok(_) => Some(v),
                        e @ Err(_) => Some(e),
                    },
                    Err(e) => Some(Err(anyhow!(
                        "unreadable directory path {:?}, with error {:?}",
                        path,
                        e
                    ))),
                },
            })
            .collect::<Result<Vec<Vec<PathBuf>>>>()?;
        let flat_paths = folded_paths.into_iter().flatten().collect();
        Ok(flat_paths)
    }
}
#[derive(Deserialize, Debug, PartialEq, Clone, Eq, Hash)]
struct Library {
    name: String,
    location: String,
}
impl Library {
    fn location_path(&self) -> Option<PathBuf> {
        let location = &self.location;
        match shellexpand::env(location) {
            Ok(expanded_location) => {
                let clean_location = expanded_location
                    .replace(r#"/"#, std::path::MAIN_SEPARATOR_STR)
                    .replace(r#"\"#, std::path::MAIN_SEPARATOR_STR)
                    .replace(r#"$|"#, "");
                match PathBuf::from_str(&clean_location) {
                    Ok(v) => Some(v),
                    Err(e) => {
                        info!("fails to convert {clean_location:?} into an owned path with error {e:?}");
                        None
                    }
                }
            }
            Err(e) => {
                info!("fails to expand library location {location:?} by env variables with error: {e:?}");
                None
            }
        }
    }
    fn parent_directory(&self) -> Option<PathBuf> {
        let path = self.location_path()?;
        match path.parent() {
            Some(parent) => Some(parent.to_owned()),
            None => {
                info!("fails to retrieve library parent directory.");
                return None;
            }
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use assert_fs::prelude::*;
    use assert_fs::{fixture::FileWriteStr, NamedTempFile, TempDir};
    use tracing::Value;
    const XML_EXAMPLE: &str = r#"<?xml version="1.0" encoding="ISO-8859-1"?>
<system xmlns="http://www.eiffel.com/developers/xml/configuration-1-16-0"
	xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
	xsi:schemaLocation="http://www.eiffel.com/developers/xml/configuration-1-16-0 http://www.eiffel.com/developers/xml/configuration-1-16-0.xsd"
	name="sanity-check" uuid="6BE01FDA-BFC4-43D8-9182-99C7A5EFA7E9">
	<target name="sanity-check">
		<root all_classes="true" />
		<file_rule>
			<exclude>/\.git$</exclude>
			<exclude>/\.svn$</exclude>
			<exclude>/CVS$</exclude>
			<exclude>/EIFGENs$</exclude>
		</file_rule>
		<capability>
			<void_safety support="all" />
		</capability>
		<cluster name="list_inversion"
			location="./list_inversion/" recursive="true" />
		<cluster name="levenshtein_distance"
			location="./levenshtein_distance/" recursive="true" />
	</target>
</system>"#;
    const XML_LIBRARY_CONFIG: &str = r#"<?xml version="1.0" encoding="ISO-8859-1"?>
<system xmlns="http://www.eiffel.com/developers/xml/configuration-1-16-0"
	xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
	xsi:schemaLocation="http://www.eiffel.com/developers/xml/configuration-1-16-0 http://www.eiffel.com/developers/xml/configuration-1-16-0.xsd"
	name="sanity-check" uuid="6BE01FDA-BFC4-43D8-9182-99C7A5EFA7E9">
	<target name="sanity-check">
		<root all_classes="true" />
		<file_rule>
			<exclude>/\.git$</exclude>
			<exclude>/\.svn$</exclude>
			<exclude>/CVS$</exclude>
			<exclude>/EIFGENs$</exclude>
		</file_rule>
		<capability>
			<void_safety support="all" />
		</capability>
		<cluster name="lib"
			location="./lib/" recursive="true" />
	</target>
</system>"#;
    const XML_EXAMPLE_WITH_LIBRARY: &str = r#"<?xml version="1.0" encoding="ISO-8859-1"?>
<system xmlns="http://www.eiffel.com/developers/xml/configuration-1-16-0"
	xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
	xsi:schemaLocation="http://www.eiffel.com/developers/xml/configuration-1-16-0 http://www.eiffel.com/developers/xml/configuration-1-16-0.xsd"
	name="sanity-check" uuid="6BE01FDA-BFC4-43D8-9182-99C7A5EFA7E9">
	<target name="sanity-check">
		<root all_classes="true" />
		<file_rule>
			<exclude>/\.git$</exclude>
			<exclude>/\.svn$</exclude>
			<exclude>/CVS$</exclude>
			<exclude>/EIFGENs$</exclude>
		</file_rule>
		<capability>
			<void_safety support="all" />
		</capability>
		<library name="base" location="$AP/library_config.ecf" />
		<cluster name="list_inversion"
			location="./list_inversion/" recursive="true" />
		<cluster name="levenshtein_distance"
			location="./levenshtein_distance/" recursive="true" />
	</target>
</system>
"#;
    const XML_EXAMPLE_NESTED_CLUSTERS: &str = r#"<?xml version="1.0" encoding="ISO-8859-1"?>
<system xmlns="http://www.eiffel.com/developers/xml/configuration-1-16-0"
	xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance"
	xsi:schemaLocation="http://www.eiffel.com/developers/xml/configuration-1-16-0 http://www.eiffel.com/developers/xml/configuration-1-16-0.xsd"
	name="sanity-check" uuid="6BE01FDA-BFC4-43D8-9182-99C7A5EFA7E9">
	<target name="sanity-check">
		<root all_classes="true" />
		<file_rule>
			<exclude>/\.git$</exclude>
			<exclude>/\.svn$</exclude>
			<exclude>/CVS$</exclude>
			<exclude>/EIFGENs$</exclude>
		</file_rule>
		<capability>
			<void_safety support="all" />
		</capability>
		<cluster name="list_inversion" location="./list_inversion/">
			<cluster name="nested" location="nested/"/>
		</cluster>
	</target>
</system>
"#;
    #[test]
    fn parse_cluster() {
        let system: System = serde_xml_rs::from_str(XML_EXAMPLE).unwrap();
        let target = system.target;
        let cluster = target.cluster.first().expect("At least a cluster");
        assert_eq!(cluster.name, "list_inversion".to_string());
        assert_eq!(cluster.location, "./list_inversion/".to_string());
        assert!(cluster.recursive.is_some_and(|x| x));
    }
    #[test]
    fn parse_library() {
        let system: System = serde_xml_rs::from_str(XML_EXAMPLE_WITH_LIBRARY).unwrap();
        let target = system.target;
        let libraries = target.library.expect("Library is present");
        let library = libraries.first().expect("At least a library");
        assert_eq!(library.name, "base".to_string());
        assert_eq!(library.location, "$AP/library_config.ecf".to_string());
    }
    #[test]
    fn expand_path_containing_environment_variables() {
        let path_with_environment_variables = "$AP/library_config.ecf".to_string();
        std::env::set_var("AP", std::env::temp_dir());
        let expanded_path = shellexpand::env(&path_with_environment_variables)
            .expect("Expansion of library location");
        let path = std::env::temp_dir().join(std::path::PathBuf::from("library_config.ecf"));
        assert_eq!(
            expanded_path,
            path.to_str()
                .expect("Path to string (might fail on windows)")
        )
    }
    #[test]
    fn all_clusters() -> anyhow::Result<()> {
        let ap_val = std::env::temp_dir();
        std::env::set_var("AP", &ap_val);
        let system: System = serde_xml_rs::from_str(XML_EXAMPLE_WITH_LIBRARY)
            .expect("Parsable {XML_EXAMPLE_LIBRARY}");
        let lib = system
            .target
            .library
            .clone()
            .ok_or(anyhow!("Fail to parse libraries"))?
            .first()
            .ok_or(anyhow!("No library parsed"))?
            .clone();

        let lib_path = lib.location.clone();

        let file = NamedTempFile::new(
            shellexpand::env(lib_path.as_str())
                .context("Expand library location into valid path")?
                .as_ref(),
        )
        .context("Create named temp file")?;
        file.write_str(XML_LIBRARY_CONFIG)
            .context("Write to temp file")?;

        let library_path = ap_val
            .join("./lib/")
            .to_str()
            .context("Generated value for env variable AP cannot be converted to string")?
            .to_owned();
        assert!(system.clone().clusters().contains(&Cluster {
            location: library_path,
            name: "lib".to_string(),
            recursive: Some(true),
            cluster: None
        }));
        assert!(system.clusters().contains(&Cluster {
            name: "levenshtein_distance".to_string(),
            location: "./levenshtein_distance/".to_string(),
            recursive: Some(true),
            cluster: None
        }));
        Ok(())
    }
    #[test]
    fn cluster_eiffel_files() -> anyhow::Result<()> {
        let temp_dir = TempDir::new().context("failed to create temp dir")?;
        let file_name_and_ext = "test.e";
        temp_dir
            .child(file_name_and_ext)
            .touch()
            .context("failed to create empty file in temp directory")?;
        let path = temp_dir
            .path()
            .to_str()
            .ok_or(anyhow!("failed conversion of path to string"))?
            .to_owned();
        let c = Cluster {
            name: "test".to_string(),
            location: path,
            recursive: Some(false),
            cluster: None,
        };
        let eiffel_files = c.eiffel_files().context("Cluster eiffel files")?;
        eprintln!("{:?}", eiffel_files.first());
        assert_eq!(eiffel_files.len(), 1);
        assert_eq!(
            eiffel_files.iter().next().unwrap(),
            &temp_dir.path().join(file_name_and_ext)
        );
        Ok(())
    }
    #[test]
    fn nested_cluster() -> anyhow::Result<()> {
        let system: System = serde_xml_rs::from_str(XML_EXAMPLE_NESTED_CLUSTERS)?;
        let clusters = system.target.cluster;
        assert_eq!(
            clusters,
            vec![Cluster {
                name: "list_inversion".to_string(),
                location: "./list_inversion/".to_string(),
                recursive: None,
                cluster: Some(vec![Cluster {
                    name: "nested".to_string(),
                    location: "nested/".to_string(),
                    recursive: None,
                    cluster: None
                }])
            }]
        );
        Ok(())
    }
}
