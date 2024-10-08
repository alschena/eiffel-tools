use anyhow::{Context, Result};
use serde::Deserialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use walkdir::{self, DirEntryExt};
#[derive(Deserialize, Debug, PartialEq, Clone, Eq)]
struct Config {
    system: System,
}
#[derive(Deserialize, Debug, PartialEq, Eq, Clone)]
struct System {
    target: Target,
}
impl System {
    /// All clusters the ones defined in the target and the ones defined in the library.
    fn clusters(self) -> Result<Vec<Cluster>> {
        let mut clusters: Vec<Cluster> = self.target.cluster;
        match self.target.library {
            Some(lib) => {
                for l in lib.into_iter() {
                    let path = PathBuf::from(shellexpand::env(&l.location)?.as_ref());
                    let xml_config = std::fs::read_to_string(path)
                        .context(format!("read from {:?}", shellexpand::env(&l.location)))?;
                    let system: System = serde_xml_rs::from_str(xml_config.as_str())
                        .context("Library files store an eiffel system")?;
                    for c in system.target.cluster {
                        clusters.push(c);
                    }
                }
                Ok(clusters)
            }
            None => Ok(clusters),
        }
    }
    /// All eiffel files present in the system.
    fn eiffel_files(self) -> Result<Vec<PathBuf>> {
        let mut eiffel_files: Vec<PathBuf> = Vec::new();
        for cluster in self
            .clusters()
            .context("All clusters in self.")?
            .into_iter()
        {
            eiffel_files.append(&mut cluster.eiffel_files()?);
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
    recursive: bool,
}
impl Cluster {
    fn eiffel_files(&self) -> Result<Vec<PathBuf>> {
        let shell_expanded_string = shellexpand::env(&self.location)?;
        let path = PathBuf::from(shell_expanded_string.as_ref());
        let mut res = Vec::new();
        match self.recursive {
            true => {
                for entry in walkdir::WalkDir::new(path).into_iter() {
                    let entry = match entry.context("Entry in recursive walk is invalid") {
                        Ok(e) => e,
                        Err(_) => continue,
                    };
                    match entry.path().extension() {
                        Some(ext) if ext == "e" => res.push(entry.path().to_owned()),
                        _ => continue,
                    }
                }
            }
            false => {
                for entry in fs::read_dir(path)?.into_iter() {
                    let entry = match entry.context("Entry in recursive walk is invalid") {
                        Ok(e) => e,
                        Err(_) => continue,
                    };
                    match entry.path().extension() {
                        Some(ext) if ext == "e" => res.push(entry.path().to_owned()),
                        _ => continue,
                    }
                }
            }
        }
        Ok(res)
    }
}
#[derive(Deserialize, Debug, PartialEq, Clone, Eq, Hash)]
struct Library {
    name: String,
    location: String,
}
#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path;
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
    #[test]
    fn extract_cluster() {
        let system: System = serde_xml_rs::from_str(XML_EXAMPLE).unwrap();
        let target = system.target;
        let cluster = target.cluster.first().expect("At least a cluster");
        assert_eq!(cluster.name, "list_inversion".to_string());
        assert_eq!(cluster.location, "./list_inversion/".to_string());
        assert!(cluster.recursive);
    }
    #[test]
    fn extract_library() {
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
        let system: System = serde_xml_rs::from_str(XML_EXAMPLE_WITH_LIBRARY)
            .expect("Parsable {XML_EXAMPLE_LIBRARY}");
        match system.target.library.clone() {
            Some(lib) => {
                for loc in lib.iter() {
                    let file = NamedTempFile::new(
                        shellexpand::env(loc.location.as_str())
                            .expect("Expand library location into valid path")
                            .as_ref(),
                    )
                    .context("Create named temp file")?;
                    file.write_str(XML_LIBRARY_CONFIG)
                        .expect("Write to temp file");
                }
            }
            None => panic!("Parsable library"),
        }
        assert!(system
            .clone()
            .clusters()
            .context("All clusters location")?
            .contains(&Cluster {
                location: "./lib/".to_string(),
                name: "lib".to_string(),
                recursive: true,
            }));
        assert!(system
            .clusters()
            .context("All clusters location")?
            .contains(&Cluster {
                name: "levenshtein_distance".to_string(),
                location: "./levenshtein_distance/".to_string(),
                recursive: true
            }));
        Ok(())
    }
        assert_eq!(
            library.location,
            "$AP/library/base/base-scoop-safe.ecf".to_string()
        );

        let library_path =
            shellexpand::env(&library.location).expect("Expansion of library location");
        let library_path = path::Path::new(library_path.as_ref());
        let library_config = fs::read_to_string(path::Path::new(&library_path))
            .expect("The library location must be a valid path.");
        let library_system: System = serde_xml_rs::from_str(&library_config).unwrap();
        let library_target = library_system.target;
        let _cluster = library_target
            .cluster
            .first()
            .expect("There is at least a cluster in the library config file");
    }
}
