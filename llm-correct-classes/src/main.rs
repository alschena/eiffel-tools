use clap::Parser;
use eiffel_tools_lib::code_entities::prelude::*;
use eiffel_tools_lib::config::System;
use eiffel_tools_lib::generators::Generators;
use eiffel_tools_lib::language_server_protocol::commands::class_wide_feature_fixes;
use eiffel_tools_lib::workspace::Workspace;
use std::path::Path;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    config_file: std::path::PathBuf,
    #[arg(long)]
    classes_file: std::path::PathBuf,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    class_by_class(Args::parse()).await;

    println!("DONE FIXING CLASSES.");
}

async fn class_by_class(
    Args {
        config_file,
        classes_file,
    }: Args,
) {
    let system = system(&config_file);
    let workspace = Arc::new(RwLock::new(Workspace::default()));

    load_workspace(system, workspace.clone()).await;

    let classes_names = name_classes(&classes_file).await;

    let mut generators = Generators::default();
    generators.add_new().await;

    let mut ws = workspace.write().await;
    for class_name in &classes_names {
        class_wide_feature_fixes::fix_class_in_place(&generators, &mut ws, class_name).await;
    }
}

async fn load_workspace(system: System, workspace: Arc<RwLock<Workspace>>) {
    let parsing_handle = tokio::spawn(async move {
        let mut ws = workspace.write().await;
        ws.load_system(&system).await;
    });

    let _ = parsing_handle
        .await
        .inspect_err(|e| eprintln!("awaiting parsing fails with:{:#?} ", e));
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
        .inspect_err(|e| eprintln!("fails to read classes_file with error: {:#?}", e))
        .ok()
        .and_then(|text| {
            String::from_utf8(text)
                .inspect_err(|e| {
                    eprintln!(
                        "fails to convert content of classes file to UFT8 string with error: {:#?}",
                        e
                    )
                })
                .ok()
        })
        .map(|text| {
            text.lines()
                .flat_map(|name| (!name.is_empty()).then_some(ClassName(name.to_uppercase())))
                .inspect(|name| println!("Class name read: {}", name))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default()
}
