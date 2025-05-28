use serde::Deserialize;
use std::fs::{self, canonicalize};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use tracing::info;
use walkdir::{self};
#[derive(Deserialize, Debug, PartialEq, Clone, Eq)]
pub struct Config {
    pub system: System,
}
#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct System {
    target: Target,
}
impl System {
    pub fn parse_from_file(file: &Path) -> Option<System> {
        match std::fs::read_to_string(file) {
            Ok(v) => match quick_xml::de::from_str(v.as_str()) {
                Ok(v) => Some(v),
                Err(e) => {
                    info!("fails to parse the configuration file of the library with error {e:?}");
                    None
                }
            },
            Err(e) => {
                info!("fails reading from {file:?} with error {e:?}");
                None
            }
        }
    }
    /// All eiffel files present in the system.
    pub fn eiffel_files(&self) -> Vec<PathBuf> {
        let eiffel_files = self
            .target
            .cluster
            .iter()
            .filter_map(|cluster| cluster.eiffel_files())
            .flatten()
            .collect::<Vec<PathBuf>>();

        let Some(ref libraries) = self.target.library else {
            return eiffel_files;
        };

        libraries.iter().fold(eiffel_files, |mut acc, library| {
            let Some(path) = library.path() else {
                return acc;
            };
            let Some(system) = System::parse_from_file(&path) else {
                info!("fails to parse library system at {path:?}");
                return acc;
            };
            library
                .eiffel_files(&system.target.cluster)
                .for_each(|file| acc.push(file));
            acc
        })
    }
}
#[derive(Deserialize, Debug, PartialEq, Clone, Eq, Hash)]
struct Target {
    cluster: Vec<Cluster>,
    library: Option<Vec<Library>>,
}
#[derive(Deserialize, Debug, PartialEq, Clone, Eq, Hash)]
enum Name {
    #[serde(rename = "@name")]
    Attribute(String),
    #[serde(rename = "name")]
    Field(String),
}
impl std::ops::Deref for Name {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        match self {
            Name::Attribute(s) => s,
            Name::Field(s) => s,
        }
    }
}
#[derive(Deserialize, Debug, PartialEq, Clone, Eq, Hash)]
struct Cluster {
    #[serde(flatten)]
    name: Name,
    #[serde(rename = "@location")]
    location: String,
    #[serde(rename = "@recursive")]
    recursive: Option<bool>,
    cluster: Option<Vec<Cluster>>,
}
impl Cluster {
    fn recursive(&self) -> bool {
        matches!(self.recursive, Some(true))
    }

    fn path(&self) -> Option<PathBuf> {
        match shellexpand::env(&self.location) {
            Ok(v) => Some(PathBuf::from(
                v.replace(r#"/"#, std::path::MAIN_SEPARATOR_STR)
                    .replace(r#"\"#, std::path::MAIN_SEPARATOR_STR)
                    .replace(r#"$|"#, ""),
            )),
            Err(e) => {
                info! {"fails to expand cluster location with environmental variables with error {e:?}"};
                None
            }
        }
    }

    fn paths(&self) -> Option<impl IntoIterator<Item = (PathBuf, bool)> + '_> {
        let base_path = self.path()?;
        let mut paths = vec![(base_path.clone(), self.recursive())];
        let Some(clusters) = self.cluster.as_ref() else {
            return Some(paths);
        };
        clusters.iter().for_each(|inner_cluster| {
            let Some(inner_path) = inner_cluster.path() else {
                return;
            };
            paths.push((base_path.join(inner_path), inner_cluster.recursive()))
        });
        Some(paths)
    }

    fn in_library_paths<'a, 'b>(
        &'a self,
        lib: &'b Library,
    ) -> Option<impl Iterator<Item = (PathBuf, bool)> + 'b>
    where
        'a: 'b,
    {
        self.paths().map(|paths| {
            paths.into_iter().filter_map(|(path, recursive)| {
                if path.is_absolute() {
                    Some((path, recursive))
                } else {
                    lib.parent_directory()
                        .map(|lib_dir| (lib_dir.join(path), recursive))
                }
            })
        })
    }
    fn eiffel_files_from_paths(
        paths: impl Iterator<Item = (PathBuf, bool)>,
    ) -> impl IntoIterator<Item = PathBuf> {
        paths
            .filter_map(|(path, recursive)| match canonicalize(&path) {
                Ok(path) => Some((path, recursive)),
                Err(e) => {
                    info!("fails to canonicalize path {path:?} with error {e:?}");
                    None
                }
            })
            .fold(Vec::new(), |mut acc: Vec<PathBuf>, (path, recursive)| {
                assert!(path.is_absolute());
                if recursive {
                    walkdir::WalkDir::new(path)
                        .into_iter()
                        .filter_map(|entry| {
                            let Ok(entry) = entry else {
                                return None;
                            };
                            let path = entry.path();
                            if path.extension().is_some_and(|ext| ext == "e") {
                                Some(path.to_owned())
                            } else {
                                None
                            }
                        })
                        .for_each(|path| acc.push(path));
                } else {
                    let entries = match fs::read_dir(&path) {
                        Ok(v) => v,
                        Err(ref e) => {
                            info!(
                                "fails to read entries of directory at {path:?} with error {e:?}"
                            );
                            return acc;
                        }
                    };
                    entries
                        .filter_map(|entry| match entry {
                            Ok(v) if (v.path()).extension().is_some_and(|ext| ext == "e") => {
                                Some(v.path().to_owned())
                            }
                            Ok(_) => None,
                            Err(ref e) => {
                                info!("fails to read entry {entry:?} with error {e:?}");
                                None
                            }
                        })
                        .for_each(|path| acc.push(path));
                }
                acc
            })
    }

    fn eiffel_files(&self) -> Option<impl IntoIterator<Item = PathBuf> + use<'_>> {
        self.paths()
            .map(|paths| Cluster::eiffel_files_from_paths(paths.into_iter()))
    }
}
#[derive(Deserialize, Debug, PartialEq, Clone, Eq, Hash)]
struct Library {
    #[serde(flatten)]
    name: Name,
    #[serde(rename = "@location")]
    location: String,
}
impl Library {
    fn path(&self) -> Option<PathBuf> {
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
        let path = self.path()?;
        match path.parent() {
            Some(parent) => Some(parent.to_owned()),
            None => {
                info!("fails to retrieve library parent directory.");
                None
            }
        }
    }
    fn eiffel_files<'a, 'b: 'a>(
        &'a self,
        clusters: &'b [Cluster],
    ) -> impl Iterator<Item = PathBuf> + 'a {
        clusters
            .iter()
            .filter_map(move |cluster| {
                let paths = cluster.in_library_paths(self)?;
                Some(Cluster::eiffel_files_from_paths(paths))
            })
            .flatten()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;
    use assert_fs::prelude::*;
    use assert_fs::{fixture::FileWriteStr, NamedTempFile, TempDir};
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
        let system: System = quick_xml::de::from_str(XML_EXAMPLE).unwrap();
        let target = system.target;
        let cluster = target.cluster.first().expect("At least a cluster");
        assert_eq!(*cluster.name, "list_inversion".to_string());
        assert_eq!(cluster.location, "./list_inversion/".to_string());
        assert!(cluster.recursive.is_some_and(|x| x));
    }
    #[test]
    fn parse_library() {
        let system: System = quick_xml::de::from_str(XML_EXAMPLE_WITH_LIBRARY).unwrap();
        let target = system.target;
        let libraries = target.library.expect("Library is present");
        let library = libraries.first().expect("At least a library");
        assert_eq!(*library.name, "base".to_string());
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
    fn all_clusters() {
        let ap_val = std::env::temp_dir();
        std::env::set_var("AP", &ap_val);
        let system: System = quick_xml::de::from_str(XML_EXAMPLE_WITH_LIBRARY)
            .expect("Parsable {XML_EXAMPLE_LIBRARY}");
        let Some(libraries) = system.target.library else {
            panic!("fails to parse libraries from {:?}", system.target.library)
        };
        let Some(library) = libraries.first() else {
            panic!("fails to register valid library from {libraries:?}")
        };
        let Some(library_path) = library.path() else {
            panic!("fails to retrieve path from library {library:?}")
        };
        assert_eq!(
            library_path,
            PathBuf::from(ap_val.join("library_config.ecf"))
        );

        let file = NamedTempFile::new(&library_path).expect("fails to create named temp file");
        file.write_str(XML_LIBRARY_CONFIG)
            .expect("fails to write to temp file");
        let mut local_clusters = system.target.cluster.iter();
        assert_eq!(
            local_clusters.next(),
            Some(&Cluster {
                name: Name::Attribute("list_inversion".to_string()),
                location: "./list_inversion/".to_string(),
                recursive: Some(true),
                cluster: None
            })
        );
        assert_eq!(
            local_clusters.next(),
            Some(&Cluster {
                name: Name::Attribute("levenshtein_distance".to_string()),
                location: "./levenshtein_distance/".to_string(),
                recursive: Some(true),
                cluster: None
            })
        );
        let Some(library_system) = System::parse_from_file(&library_path) else {
            panic!("fails to parse library system")
        };
        let remote_clusters = library_system.target.cluster;
        let Some(first_remote) = remote_clusters.first() else {
            panic!("fails to find any remote cluster.")
        };
        assert_eq!(
            first_remote,
            &Cluster {
                location: "./lib/".to_string(),
                name: Name::Attribute("lib".to_string()),
                recursive: Some(true),
                cluster: None
            }
        );
    }
    #[test]
    fn local_eiffel_files() {
        let temp_dir = TempDir::new().expect("fails to create temporary directory.");
        let filepath = temp_dir.child("test.e");
        filepath
            .touch()
            .expect("fails to create empty file in temp directory");
        assert!(filepath.exists());
        let location: String = temp_dir.to_string_lossy().into();
        eprintln!("location: {location:?}");
        let c = Cluster {
            name: Name::Attribute("test".to_string()),
            location,
            recursive: Some(false),
            cluster: None,
        };
        let Some(eiffel_files) = c.eiffel_files() else {
            panic!("fails to find eiffel files");
        };
        let mut iterator = eiffel_files.into_iter();
        let Some(first_file) = iterator.next() else {
            panic!("fails to retrieve first eiffel file")
        };
        assert!(
            iterator.next().is_none(),
            "there should be exaclty one eiffel file."
        );
        assert_eq!(first_file, temp_dir.path().join("test.e"));
    }
    #[test]
    fn nested_cluster() -> anyhow::Result<()> {
        let system: System = quick_xml::de::from_str(XML_EXAMPLE_NESTED_CLUSTERS)?;
        let clusters = system.target.cluster;
        assert_eq!(
            clusters,
            vec![Cluster {
                name: Name::Attribute("list_inversion".to_string()),
                location: "./list_inversion/".to_string(),
                recursive: None,
                cluster: Some(vec![Cluster {
                    name: Name::Attribute("nested".to_string()),
                    location: "nested/".to_string(),
                    recursive: None,
                    cluster: None
                }])
            }]
        );
        Ok(())
    }
    #[test]
    fn xml_out_of_order_library_parsing() -> anyhow::Result<()> {
        let xml_with_out_of_order_library_entries: &str = r#"<?xml version="1.0" encoding="ISO-8859-1"?>
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
    		<capability> <void_safety support="all" /> </capability>
    		<library name="base" location="$AP/library_config.ecf" />
    		<cluster name="list_inversion" location="./list_inversion/" recursive="true" />
    		<library name="base32" location="$AP/another/library_config.ecf" />
    		<cluster name="levenshtein_distance" location="./levenshtein_distance/" recursive="true" />
    	</target>
    </system>
    "#;
        let sys: System = quick_xml::de::from_str(xml_with_out_of_order_library_entries)
            .map_err(|e| anyhow!("fails to parse out of order xml with error: {e:?}"))?;
        let Some(libraries) = sys.target.library else {
            return Err(anyhow!(
                "fails to find any library in xml with out of order libraries"
            ));
        };
        let mut libs = libraries.iter();
        assert!(libs.next().is_some());
        assert!(libs.next().is_some());
        Ok(())
    }
}
