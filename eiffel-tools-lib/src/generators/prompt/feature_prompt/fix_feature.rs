use super::*;

impl FeaturePrompt {
    pub fn default_for_routine_fixes() -> Self {
        Self {
            system_message: (String::from(
                r#"You are a coding assistant, expert in the Eiffel programming language in writing formally verified code.
You have extensive training in the usage of AutoProof, the static verifier of Eiffel.
You will receive an eiffel snippet with a comment identifying the routine to fix which contains an error message of AutoProof.
Respond with a correct version of the same routine.
"#,
            )),
            source: String::new(),
            injections: Vec::new(),
        }
    }

    pub async fn for_feature_fixes(
        feature: &Feature,
        filepath: &Path,
        error_message: String,
    ) -> Result<Self> {
        let mut var = Self::default_for_routine_fixes();
        var.set_feature_src(feature, filepath).await?;
        var.add_commented_injection_after_feature(feature, error_message)
            .await;
        var.add_commented_injection_at_the_beginning("The following feature does not verify, there is a bug in the body in its body. Fix the body of following feature. After the code you will find a comment with the error message from AutoProof.");
        Ok(var)
    }
}
