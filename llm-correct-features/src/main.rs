use clap::Parser;
use eiffel_tools_lib::code_entities::prelude::*;
use eiffel_tools_lib::config::System;
use eiffel_tools_lib::generators::FixPromptParts;
use eiffel_tools_lib::generators::Generators;
use eiffel_tools_lib::language_server_protocol::commands::fix_routine_in_place;
use eiffel_tools_lib::language_server_protocol::commands::fix_routine_in_place::LlmInteraction;
use eiffel_tools_lib::tracing::info;
use eiffel_tools_lib::tracing::warn;
use eiffel_tools_lib::tracing_subscriber::filter;
use eiffel_tools_lib::tracing_subscriber::fmt;
use eiffel_tools_lib::tracing_subscriber::fmt::format::FmtSpan;
use eiffel_tools_lib::tracing_subscriber::prelude::*;
use eiffel_tools_lib::tracing_subscriber::{Layer, Registry};
use eiffel_tools_lib::workspace::Workspace;
use futures::stream::{FuturesUnordered, StreamExt};
use serde::Serialize;
use std::io::Write;
use std::path::Path;
use std::time::SystemTime;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Debug, Clone)]
enum ClassOrFeature {
    Class(ClassName),
    ClassAndFeature(ClassName, String),
}

#[derive(clap::ValueEnum, Clone, Debug, Default)]
enum Provider {
    #[default]
    Openrouter,
    Constructor,
}

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    config: std::path::PathBuf,
    #[arg(long)]
    classes: std::path::PathBuf,
    #[arg(long, help = "LLM model name to use (e.g., 'claude-sonnet-4-0', 'gpt-4o-mini', 'o3-mini')")]
    model: Option<String>,
    #[arg(long, default_value = "openrouter", help = "LLM provider to use")]
    provider: Provider,
    #[arg(long, help = "Show verbose output including verification attempts and code changes on stderr")]
    verbose: bool,
    // Prompt part toggles (all enabled by default; pass flag to disable)
    #[arg(long, help = "Omit 'The following feature does not verify' instruction")]
    no_task_instruction: bool,
    #[arg(long, help = "Omit 'Only modify body/locals' constraint reminder")]
    no_modification_constraints: bool,
    #[arg(long, help = "Omit class invariant from prompt context")]
    no_class_invariant: bool,
    #[arg(long, help = "Omit precondition identifier list from prompt context")]
    no_precondition_identifiers: bool,
    #[arg(long, help = "Omit postcondition identifier list from prompt context")]
    no_postcondition_identifiers: bool,
    #[arg(long, help = "Omit AutoProof error message from prompt")]
    no_error_message: bool,
    #[arg(long, help = "Omit verbatim feature signature from output-format instruction")]
    no_verbatim_signature: bool,
    #[arg(long, help = "Omit Eiffel syntax reference for contracts and loops")]
    no_syntax_guide: bool,
}

#[derive(Serialize)]
struct FeatureReport {
    class_name: String,
    feature_name: String,
    model: String,
    llm_interactions: u32,
    success: bool,
    max_retries_reached: bool,
    final_status: String,
    interactions: Vec<LlmInteraction>,
    #[serde(rename = "total_elapsed_time_seconds")]
    total_elapsed_time_seconds: f64,
    completed_at: u64,
}

#[tokio::main(flavor = "current_thread")]
async fn main() {
    add_logging();
    feature_by_feature(Args::parse()).await;

    info!("DONE FIXING CLASSES.");
}

fn add_logging() {
    let log_directory_path = &Path::new(".lsp_eiffel.d");
    if !log_directory_path.exists() {
        std::fs::DirBuilder::new()
            .create(log_directory_path)
            .expect("Fails to create log directory.");
    }

    let default_log_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(log_directory_path.join("log.log"))
        .expect("Fails to create `log.log`");

    let llm_log_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(log_directory_path.join("llm.log"))
        .expect("Fails to create `llm.log`");

    let autoproof_log_file = std::fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(log_directory_path.join("autoproof.log"))
        .expect("Fails to create autoproof log file.");

    let default_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false)
        .with_writer(default_log_file)
        .with_filter(
            filter::Targets::default()
                .with_default(filter::LevelFilter::INFO)
                .with_target("llm", filter::LevelFilter::OFF),
        );

    let llm_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false)
        .with_writer(llm_log_file)
        .with_filter(filter::Targets::default().with_target("llm", filter::LevelFilter::INFO));

    let autoproof_layer = fmt::layer()
        .with_span_events(FmtSpan::CLOSE)
        .with_ansi(false)
        .with_writer(autoproof_log_file)
        .with_filter(
            filter::Targets::default().with_target("autoproof", filter::LevelFilter::INFO),
        );

    Registry::default()
        .with(default_layer)
        .with(llm_layer)
        .with(autoproof_layer)
        .init();
}

async fn feature_by_feature(
    Args {
        config: config_file,
        classes: classes_file,
        model: model_name,
        provider,
        verbose,
        no_task_instruction,
        no_modification_constraints,
        no_class_invariant,
        no_precondition_identifiers,
        no_postcondition_identifiers,
        no_error_message,
        no_verbatim_signature,
        no_syntax_guide,
    }: Args,
) {
    let system = system(&config_file);
    let workspace = Arc::new(RwLock::new(Workspace::default()));

    load_workspace(system, workspace.clone()).await;

    let classes_and_features = name_classes(&classes_file).await;

    let generators = {
        let mut generators = if let Some(ref name) = model_name {
            Generators::with_model_name(name)
        } else {
            Generators::default()
        };
        match provider {
            Provider::Openrouter => generators.add_openrouter(),
            Provider::Constructor => generators.add_constructor().await,
        }
        Arc::new(generators)
    };

    // Capture the model name before spawning tasks
    let model_name_str = generators.model_name().to_string();

    let fix_prompt_parts = FixPromptParts {
        task_instruction: !no_task_instruction,
        modification_constraints: !no_modification_constraints,
        class_invariant: !no_class_invariant,
        precondition_identifiers: !no_precondition_identifiers,
        postcondition_identifiers: !no_postcondition_identifiers,
        error_message: !no_error_message,
        verbatim_signature: !no_verbatim_signature,
        syntax_guide: !no_syntax_guide,
    };

    let classes_and_routines = {
        let ws = workspace.read().await;
        classes_and_routines(&ws, classes_and_features)
    };

    let mut handles: FuturesUnordered<_> = classes_and_routines
        .into_iter()
        .flat_map(|(classname, features)| {
            features
                .into_iter()
                .map(move |feature| (classname.clone(), feature.name().to_owned()))
        })
        .map(move |(classname, featurename)| {
            let local_generators = generators.clone();
            let local_owned_workspace = workspace.clone();
            let local_classname = classname.clone();
            let local_featurename = featurename.clone();
            let local_verbose = verbose;
            let local_parts = fix_prompt_parts.clone();

            tokio::spawn(async move {
                let mut ws = local_owned_workspace.write().await;
                eprintln!(">> {}.{}", local_classname, local_featurename);
                let result = fix_routine_in_place::fix_routine_in_place(
                    &local_generators,
                    &mut ws,
                    &local_classname,
                    &local_featurename,
                    local_verbose,
                    local_parts,
                )
                .await;
                (local_classname, local_featurename, result)
            })
        })
        .collect();

    // Process results as they complete (not in order)
    while let Some(handle_result) = handles.next().await {
        let (classname, featurename, result) = handle_result.expect("Fails to await fix routine in place.");
        let completed_at = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        let report = FeatureReport {
            class_name: classname.to_string(),
            feature_name: featurename.to_string(),
            model: model_name_str.clone(),
            llm_interactions: result.llm_interactions,
            success: result.success,
            max_retries_reached: result.max_retries_reached,
            final_status: result.final_status,
            interactions: result.interactions,
            total_elapsed_time_seconds: result.total_elapsed_time_seconds,
            completed_at,
        };
        
        // Output each feature report as a JSON line as soon as it's ready
        let json_output = serde_json::to_string(&report)
            .expect("Failed to serialize report to JSON");
        println!("{}", json_output);
        std::io::stdout().flush().expect("Failed to flush stdout");
    }
}

async fn load_workspace(system: System, workspace: Arc<RwLock<Workspace>>) {
    let parsing_handle = tokio::spawn(async move {
        let mut ws = workspace.write().await;
        ws.load_system(&system).await;
    });

    let _ = parsing_handle
        .await
        .inspect_err(|e| warn!("Parsing fails to return with:{:#?} ", e));
}

fn system(config_file: &Path) -> System {
    match config_file.extension() {
        Some(ext) if ext == "ecf" => System::parse_from_file(&config_file).unwrap_or_else(|| {
            panic!("fails to parse eiffel system from ecf: {:#?}", &config_file)
        }),
        _ => panic!("the config file must be an eiffel `ecf` file"),
    }
}

async fn name_classes(classes_file: &Path) -> Vec<ClassOrFeature> {
    tokio::fs::read(classes_file)
        .await
        .inspect_err(|e| warn!("fails to read classes_file with error: {:#?}", e))
        .ok()
        .and_then(|text| {
            String::from_utf8(text)
                .inspect_err(|e| {
                    warn!(
                        "fails to convert content of classes file to UFT8 string with error: {:#?}",
                        e
                    )
                })
                .ok()
        })
        .map(|text| {
            text.lines()
                .filter_map(|line| {
                    if line.is_empty() {
                        return None;
                    }
                    let trimmed = line.trim();
                    if trimmed.contains('.') {
                        let parts: Vec<&str> = trimmed.splitn(2, '.').collect();
                        if parts.len() == 2 {
                            let class_name = parts[0].trim().to_uppercase();
                            let feature_name = parts[1].trim().to_string();
                            if !class_name.is_empty() && !feature_name.is_empty() {
                                info!("Class and feature read: {}.{}", class_name, feature_name);
                                return Some(ClassOrFeature::ClassAndFeature(
                                    ClassName(class_name),
                                    feature_name,
                                ));
                            }
                        }
                    }
                    // Fall back to treating as class name only
                    let class_name = trimmed.to_uppercase();
                    info!("Class name read: {}", class_name);
                    Some(ClassOrFeature::Class(ClassName(class_name)))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn classes_and_routines<'cl>(
    workspace: &'cl Workspace,
    classes_and_features: Vec<ClassOrFeature>,
) -> Vec<(ClassName, Vec<Feature>)> {
    classes_and_features
        .into_iter()
        .filter_map(|class_or_feature| {
            let (class_name, feature_filter) = match class_or_feature {
                ClassOrFeature::Class(name) => (name, None),
                ClassOrFeature::ClassAndFeature(name, feature) => (name, Some(feature)),
            };
            let path = workspace.path(&class_name);
            let all_features = workspace.class(path).map(|class| class.features())?;
            
            let features: Vec<Feature> = if let Some(ref filter_name) = feature_filter {
                // Filter to only the specified feature
                all_features
                    .iter()
                    .filter(|f| *f.name() == *filter_name)
                    .cloned()
                    .collect()
            } else {
                // Include all features
                all_features.clone()
            };
            
            if features.is_empty() {
                if let Some(ref filter_name) = feature_filter {
                    warn!(
                        "Feature '{}' not found in class '{}'",
                        filter_name, class_name
                    );
                }
                return None;
            }
            
            Some((class_name, features))
        })
        .collect()
}
