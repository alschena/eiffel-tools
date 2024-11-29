use super::*;

pub struct LLM<'a, 'b> {
    model_config: gemini::Config,
    client: reqwest::Client,
    file: Option<&'a ProcessedFile>,
    workspace: Option<&'b Workspace>,
}
impl<'a, 'b> LLM<'a, 'b> {
    pub fn set_file(&mut self, file: &'a ProcessedFile) {
        self.file = Some(file);
    }
    fn model_config(&self) -> &gemini::Config {
        &self.model_config
    }
    fn client(&self) -> &reqwest::Client {
        &self.client
    }
    fn target_url(&self) -> Result<Url, super::Error<'static>> {
        let Some(file) = self.file else {
            panic!("target file must be set in LLM.")
        };
        Url::from_file_path(file.path()).map_err(|_| {
            super::Error::PassThroughError("fails to transform path into lsp_types::Url")
        })
    }
}
impl<'a, 'b> LLM<'a, 'b> {
    fn feature_at_point_with_src(
        &self,
        point: &Point,
    ) -> Result<(&'a Feature, String), super::Error<'static>> {
        let Some(file) = self.file else {
            panic!("target file must be set in LLM.")
        };
        match file.feature_around_point(&point) {
            Some(feature) => match file.feature_src(&feature) {
                Ok(src) => Ok((feature, src)),
                Err(_) => Err(super::Error::PassThroughError(
                    "fails to extract feature source from file",
                )),
            },
            None => Err(super::Error::CodeActionDisabled(
                "There is no feature surrounding the cursor",
            )),
        }
    }
    async fn request_specification(
        &self,
        feature: &Feature,
        feature_src: &str,
    ) -> Result<RoutineSpecification, super::Error<'static>> {
        let Some(file) = self.file else {
            panic!("target file must be set in LLM.")
        };
        let Some(workspace) = self.workspace else {
            panic!("workspace must be set in LLM")
        };
        let target_model = file.class().full_model(workspace.system_classes());
        let mut request = gemini::Request::from(format!(
            "Add preconditions and postconditions to the following routine. DO NOT ADD CONTRACT CLAUSES ALREADY PRESENT.\n{}",
            feature_src
        ));
        request.set_config(gemini::GenerationConfig::from(
            RoutineSpecification::to_response_schema(),
        ));

        match request
            .process_with_async_client(self.model_config(), self.client())
            .await
        {
            Ok(response) => {
                info!("Request to llm: {request:?}\nResponse from llm: {response:?}");
                match response.parsed().next() {
                    Some(spec) => Ok(spec),
                    None => Err(super::Error::PassThroughError(
                        "No specification for routine was produced",
                    )),
                }
            }
            Err(_) => Err(super::Error::PassThroughError(
                "fails to process llm request",
            )),
        }
    }
    pub async fn add_contracts_at_point(
        &self,
        point: &Point,
        workspace: &Workspace,
    ) -> Result<WorkspaceEdit, super::Error<'static>> {
        let (feature, feature_src) = self.feature_at_point_with_src(point)?;
        let Some(precondition_insert_point) = feature.point_end_preconditions() else {
            return Err(super::Error::CodeActionDisabled(
                "Only attributes with an attribute block and routines support adding preconditions",
            ));
        };
        let Some(postcondition_insert_point) = feature.point_end_postconditions() else {
            return Err(super::Error::CodeActionDisabled("Only attributes with an attribute block and routines support adding postconditions"));
        };
        let RoutineSpecification {
            precondition: pre,
            postcondition: post,
        } = self.request_specification(feature, &feature_src).await?;

        let url = self.target_url()?;
        Ok(WorkspaceEdit::new(HashMap::from([(
            url,
            vec![
                text_edit_add_precondition(&feature, precondition_insert_point.clone(), pre),
                text_edit_add_postcondition(&feature, postcondition_insert_point.clone(), post),
            ],
        )])))
    }
}

impl<'a, 'b> Default for LLM<'a, 'b> {
    fn default() -> Self {
        Self {
            model_config: gemini::Config::default(),
            client: reqwest::Client::new(),
            file: None,
            workspace: None,
        }
    }
}
