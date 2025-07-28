use clap::Parser;
use eiffel_tools_lib::code_entities::prelude::*;
use eiffel_tools_lib::config::System;
use eiffel_tools_lib::generators::Generators;
use eiffel_tools_lib::language_server_protocol::commands::fix_routine_in_place;
use eiffel_tools_lib::tracing::info;
use eiffel_tools_lib::tracing::warn;
use eiffel_tools_lib::tracing_subscriber::filter;
use eiffel_tools_lib::tracing_subscriber::fmt;
use eiffel_tools_lib::tracing_subscriber::fmt::format::FmtSpan;
use eiffel_tools_lib::tracing_subscriber::prelude::*;
use eiffel_tools_lib::tracing_subscriber::{Layer, Registry};
use eiffel_tools_lib::workspace::Workspace;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    config: std::path::PathBuf,
    #[arg(long)]
    classes: std::path::PathBuf,
}

#[tokio::main(flavor = "multi_thread")]
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
    }: Args,
) {
    let system = system(&config_file);
    let workspace = Arc::new(RwLock::new(Workspace::default()));

    load_workspace(system, workspace.clone()).await;

    let classes_names = name_classes(&classes_file).await;

    let generators = {
        let mut generators = Generators::default();
        generators.add_new().await;
        Arc::new(generators)
    };

    let classes_and_routines = {
        let ws = workspace.read().await;
        classes_and_routines(&ws, classes_names)
    };

    let handles = classes_and_routines
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

            tokio::spawn(async move {
                let mut ws = local_owned_workspace.write().await;
                fix_routine_in_place::fix_routine_in_place(
                    &local_generators,
                    &mut ws,
                    &local_classname,
                    &local_featurename,
                )
                .await
            })
        });

    for handle in handles {
        handle.await.expect("Fails to await fix routine in place.")
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

async fn name_classes(classes_file: &Path) -> Vec<ClassName> {
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
                .flat_map(|name| (!name.is_empty()).then_some(ClassName(name.to_uppercase())))
                .inspect(|name| info!("Class name read: {}", name))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}

fn classes_and_routines<'cl>(
    workspace: &'cl Workspace,
    classes: Vec<ClassName>,
) -> Vec<(ClassName, Vec<Feature>)> {
    classes
        .into_iter()
        .filter_map(|class_name| {
            let path = workspace.path(&class_name);
            let features = workspace.class(path).map(|class| class.features())?;
            Some((class_name, features.clone()))
        })
        .collect()
}
