use crate::code_entities::prelude::*;
use contract::*;
use std::borrow::Borrow;
use std::fmt::Display;
use std::ops::Deref;

pub(crate) trait Indent {
    const INDENTATION_LEVEL: usize;
    const INDENTATION_CHARACTER: char = '\t';
    fn indentation_string() -> String {
        (0..Self::INDENTATION_LEVEL).fold(
            String::with_capacity(Self::INDENTATION_LEVEL),
            |mut acc, _| {
                acc.push(Self::INDENTATION_CHARACTER);
                acc
            },
        )
    }
}

impl<T: Indent> Indent for Block<T> {
    const INDENTATION_LEVEL: usize = T::INDENTATION_LEVEL - 1;
}

impl<T: Contract> Indent for T {
    const INDENTATION_LEVEL: usize = 3;
}

impl<T: Display + Indent + Contract + Deref<Target = Vec<Clause>>> Display for Block<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.item().is_empty() {
            write!(f, "")
        } else {
            write!(
                f,
                "{}{}\n{}",
                T::keyword(),
                &self.item,
                Self::indentation_string(),
            )
        }
    }
}

#[derive(Clone, Debug, Default)]
pub struct EiffelSource(String);

impl Deref for EiffelSource {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Display for EiffelSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&ClassParent> for EiffelSource {
    fn from(value: &ClassParent) -> Self {
        let name = &value.name;

        let undefine = if value.undefine.is_empty() {
            String::new()
        } else {
            value
                .undefine
                .iter()
                .fold(String::from("undefine"), |acc, name| {
                    format!("{acc}\n\t{name}")
                })
        };

        let redefine = if value.redefine.is_empty() {
            String::new()
        } else {
            value
                .redefine
                .iter()
                .fold(String::from("redefine"), |acc, name| {
                    format!("{acc}\n\t{name}")
                })
        };

        let rename = if value.rename.is_empty() {
            String::new()
        } else {
            value
                .rename
                .iter()
                .fold(String::from("rename"), |acc, (oldname, newname)| {
                    format!("{acc}\n\t{oldname} as {newname}")
                })
        };

        let select = if value.select.is_empty() {
            String::new()
        } else {
            value
                .select
                .iter()
                .fold(String::from("select"), |acc, name| {
                    format!("{acc}\n\t{name}")
                })
        };

        let optional_end =
            if undefine.is_empty() && redefine.is_empty() && rename.is_empty() && select.is_empty()
            {
                ""
            } else {
                "end"
            };

        EiffelSource(format!(
            "{name}\n{undefine}\n{redefine}\n{rename}\n{select}\n{optional_end}"
        ))
    }
}

impl From<(&Feature, String)> for EiffelSource {
    fn from(value: (&Feature, String)) -> Self {
        let (ft, ft_body) = value;
        let indented_body = ft_body
            .lines()
            .fold(String::new(), |acc, ln| format!("{acc}\n\t{ln}"));
        EiffelSource(format!("{ft}\ndo{indented_body}\nend"))
    }
}

impl From<&ClassLocalModel> for EiffelSource {
    fn from(value: &ClassLocalModel) -> Self {
        let mut model_declaration = value
            .names()
            .iter()
            .fold(String::from("model:"), |acc, name| format!("{acc} {name},"));
        model_declaration.pop(); // remove trailing comma
        EiffelSource(model_declaration)
    }
}

impl From<(&Class, Vec<(&Feature, String)>)> for EiffelSource {
    fn from(value: (&Class, Vec<(&Feature, String)>)) -> Self {
        let (cl, fts) = value;
        let indent = |bd: EiffelSource| -> EiffelSource {
            EiffelSource(
                bd.0.lines()
                    .fold(String::new(), |acc, ln| format!("{acc}\t{ln}\n")),
            )
        };
        let class_model = if cl.model.is_empty() {
            String::new()
        } else {
            format!("note\n{}", indent((&cl.model).into()))
        };
        let class_name = indent(EiffelSource(cl.name().to_string()));
        let inheritance_block = cl
            .parents
            .iter()
            .fold(String::from("inherit\n"), |acc, pr| {
                format!("{acc}{}", indent(pr.into()))
            });
        let features = fts
            .into_iter()
            .fold(String::from("feature\n"), |acc, (ft, bd)| {
                format!("{acc}{}", indent((ft, bd).into()))
            });

        EiffelSource(format!(
            "{class_model}class\n{class_name}{inheritance_block}{features}\nend"
        ))
    }
}

impl EiffelSource {
    pub fn subclass_redefining_features<N>(
        class_name: &ClassName,
        fts: Vec<(&Feature, String)>,
        new_name: &N,
    ) -> Self
    where
        N: Borrow<str> + ?Sized,
    {
        let name = new_name.borrow().to_uppercase();

        let features: Vec<Feature> = fts
            .clone()
            .into_iter()
            .map(|(ft, _)| ft.to_owned())
            .collect();

        let feature_names = fts.iter().map(|(ft, _)| ft.name().to_owned()).collect();

        let parent = ClassParent {
            name: class_name.to_string(),
            redefine: feature_names,
            ..Default::default()
        };

        let class = Class {
            name: ClassName(name),
            features,
            parents: vec![parent],
            ..Default::default()
        };

        (&class, fts).into()
    }

    pub fn simple_precursor_call(
        parameters_redefined_feature: &FeatureParameters,
        return_type: Option<&EiffelType>,
    ) -> String {
        let prefix = if return_type.is_some() {
            "Result := Precursor"
        } else {
            "Precursor"
        };

        let comma_separated_parameters =
            parameters_redefined_feature
                .names()
                .iter()
                .fold(String::new(), |acc, param_name| {
                    if acc.is_empty() {
                        param_name.to_string()
                    } else {
                        format!("{acc}, {param_name}")
                    }
                });

        if comma_separated_parameters.is_empty() {
            prefix.to_string()
        } else {
            format!("{} ({})", prefix, comma_separated_parameters)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::Parser;
    use anyhow::Result;

    fn class(source: &str) -> anyhow::Result<Class> {
        let mut parser = Parser::new();
        parser.class_from_source(source)
    }

    #[tokio::test]
    async fn subclass_redefining_feature() -> Result<()> {
        let cl = class(
            r#"class
    TEST
feature
    sum (x,y: INTEGER): INTEGER
        deferred
        end
end
"#,
        )?;

        let ft_redefined: Vec<(&Feature, String)> = cl
            .features
            .iter()
            .map(|ft| (ft, String::from("Result := x + y -- Redefined")))
            .collect();

        let oracle_res = format!(
            r#"
class
	TEST_POSTFIX
inherit
	TEST
	redefine
		sum
	end
feature
    sum (x: INTEGER
    y: INTEGER): INTEGER
        do
            Result := x + y -- Redefined
        end
end
"#
        );

        let res =
            EiffelSource::subclass_redefining_features(cl.name(), ft_redefined, "TEST_POSTFIX");

        let oracle_res_clean = oracle_res
            .lines()
            .map(|ln| ln.trim())
            .filter(|ln| !ln.is_empty());
        let res_clean = res.lines().map(|ln| ln.trim()).filter(|ln| !ln.is_empty());

        let same = oracle_res_clean.zip(res_clean).all(|(or, ac)| {
            eprintln!("oracle_line: {or}\nresult_line: {ac}");
            or == ac
        });
        assert!(same, "oracle_res: {oracle_res}\nres: {res}");

        Ok(())
    }
}
