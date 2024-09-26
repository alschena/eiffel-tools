use serde::Deserialize;
use serde_xml_rs;
#[derive(Deserialize, Debug, PartialEq)]
struct Cluster {
    name: String,
    location: String,
    recursive: bool,
}
#[cfg(test)]
mod tests {
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
		<!-- <option warning="true"> -->
		<!-- 	<assertions precondition="true" postcondition="true" check="true" -->
		<!-- 		invariant="true" loop="true" supplier_precondition="true" /> -->
		<!-- </option> -->
		<!-- <setting name="console_application" value="true" /> -->
		<library name="base" location="$AP/library/base/base-scoop-safe.ecf" />
		<!-- <cluster name="sanity-check" -->
		<!-- 	location="./modified_condition_decision_coverage/rewrite/" recursive="true" /> -->
		<cluster name="list_inversion"
			location="./list_inversion/" recursive="true" />
		<cluster name="levenshtein_distance"
			location="./levenshtein_distance/" recursive="true" />
	</target>
</system>"#;
}
