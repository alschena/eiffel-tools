use serde::Deserialize;
#[derive(Deserialize, Debug, PartialEq, Clone)]
struct Config {
    system: System,
}
#[derive(Deserialize, Debug, PartialEq, Clone)]
struct System {
    target: Target,
}
#[derive(Deserialize, Debug, PartialEq, Clone)]
struct Target {
    cluster: Vec<Cluster>,
    library: Option<Vec<Library>>,
}
#[derive(Deserialize, Debug, PartialEq, Clone)]
struct Cluster {
    name: String,
    location: String,
    recursive: bool,
}
#[derive(Deserialize, Debug, PartialEq, Clone)]
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
    const XML_EXAMPLE_LIBRARY: &str = r#"<?xml version="1.0" encoding="ISO-8859-1"?>
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
		<library name="base" location="$AP/library/base/base-scoop-safe.ecf" />
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
        let system: System = serde_xml_rs::from_str(XML_EXAMPLE_LIBRARY).unwrap();
        let target = system.target;
        let libraries = target.library.expect("Library is present");
        let library = libraries.first().expect("At least a library");
        assert_eq!(library.name, "base".to_string());
        assert_eq!(
            library.location,
            "$AP/library/base/base-scoop-safe.ecf".to_string()
        );
    }
    #[test]
    #[ignore]
    fn extract_clusters_in_library() {
        let system: System = serde_xml_rs::from_str(XML_EXAMPLE_LIBRARY).unwrap();
        let target = system.target;
        let libraries = target.library.expect("Library is present");
        let library = libraries.first().expect("At least a library");
        assert_eq!(library.name, "base".to_string());
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
