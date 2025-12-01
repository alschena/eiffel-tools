# Experiment automatic fixing of maple-recursive

1. Download dataset

``` sh
git clone git@github.com:CI-CSE/maple-recursive-eiffel.git
```

2. Add basic AutoProof void-safe ECF as `maple-recursive-eiffel/Ace.ecf`

``` xml
<?xml version="1.0" encoding="ISO-8859-1"?>
<system xmlns="http://www.eiffel.com/developers/xml/configuration-1-21-0" xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:schemaLocation="http://www.eiffel.com/developers/xml/configuration-1-21-0 http://www.eiffel.com/developers/xml/configuration-1-21-0.xsd" name="test_precomp-safe" uuid="566fc02c-16a0-4951-b279-0128039a5f76" library_target="base-safe">
	<target name="base-safe">
		<description>Base library</description>
		<root all_classes="true"/>
		<capability>
			<concurrency use="none"/>
		</capability>
		<library name="base-safe" location="$AP\library\base\base-safe.ecf"/>
	</target>
</system>
```


3. Compile `llm-correct-features`. In root of `eiffel-tools`:

``` sh
cargo build --release -p llm-correct-features
```

4. Add `classes.txt` with all class names

``` 
MAPLE_RECURSIVE_ABSOLUTE_1
MAPLE_RECURSIVE_ABSOLUTE_2
MAPLE_RECURSIVE_CONSEQ_1
MAPLE_RECURSIVE_CONSEQ_2
MAPLE_RECURSIVE_CONSEQ_3
MAPLE_RECURSIVE_CONSEQ_4
MAPLE_RECURSIVE_INCREMENT_1
MAPLE_RECURSIVE_INCREMENT_2
MAPLE_RECURSIVE_INCREMENT_3
MAPLE_RECURSIVE_INCREMENT_4
MAPLE_RECURSIVE_MAX_2_1
MAPLE_RECURSIVE_MAX_2_2
MAPLE_RECURSIVE_MAX_3_1
MAPLE_RECURSIVE_MAX_3_2
MAPLE_RECURSIVE_MIN_2_1
MAPLE_RECURSIVE_MIN_2_2
MAPLE_RECURSIVE_MIN_3_1
MAPLE_RECURSIVE_MIN_3_2
MAPLE_RECURSIVE_SUM_2_1
MAPLE_RECURSIVE_SUM_2_2
MAPLE_RECURSIVE_SUM_3_1
MAPLE_RECURSIVE_SUM_3_2
MAPLE_RECURSIVE_SUM_N_1
MAPLE_RECURSIVE_SUM_N_2
MAPLE_RECURSIVE_SUM_N_3
MAPLE_RECURSIVE_SUM_N_4
```

5. Set env vars `CONSTRUCTOR_APP_API_TOKEN`, `AP_COMMAND="$AP/EIFGENs/batch/F_code/ecb"`

5. Run fixing

``` sh
../../../../target/release/llm-correct-features --config Ace.ecf --classes classes.txt
```

6. Reset the fixes

``` sh
git restore .
```

