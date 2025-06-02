use clap::Parser;
use eiffel_tools_lib::code_entities::prelude::*;
use eiffel_tools_lib::config::System;
use eiffel_tools_lib::generators::Generators;
use eiffel_tools_lib::language_server_protocol::commands::Command;
use eiffel_tools_lib::language_server_protocol::commands::FixRoutine;
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
    let Args {
        config_file,
        classes_file,
    } = Args::parse();

    let system = match config_file.extension() {
        Some(ext) if ext == "ecf" => System::parse_from_file(&config_file).unwrap_or_else(|| {
            panic!("fails to parse eiffel system from ecf: {:#?}", &config_file)
        }),
        _ => panic!("the config file must be an eiffel `ecf` file"),
    };

    let workspace = Arc::new(RwLock::new(Workspace::default()));

    let ws = workspace.clone();
    let parsing_handle = tokio::spawn(async move {
        let mut ws = ws.write().await;
        ws.load_system(&system).await;
    });

    let _ = parsing_handle
        .await
        .inspect_err(|e| eprintln!("awaiting parsing fails with:{:#?} ", e));

    let classes_names = name_classes(&classes_file).await;

    let ws = workspace.clone();

    let mut generators = Generators::default();
    generators.add_new().await;

    let ws = ws.read().await;

    let paths_and_routines = paths_and_routines(&ws, classes_names);
    let mut routine_fixers = routine_fixers(&ws, paths_and_routines);
    let fixes = fixes(&generators, &mut routine_fixers).await;
    print_fixes(fixes);

    println!("DONE FIXING CLASSES.");
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

fn paths_and_routines(
    workspace: &Workspace,
    classes: Vec<ClassName>,
) -> Vec<(&Path, &Vec<Feature>)> {
    classes
        .iter()
        .filter_map(|class_name| {
            let path = workspace.path(class_name);
            let features = workspace.class(path).map(|class| class.features())?;
            Some((path, features))
        })
        .collect()
}

fn routine_fixers<'ws>(
    workspace: &'ws Workspace,
    paths_and_routines: Vec<(&Path, &Vec<Feature>)>,
) -> Vec<FixRoutine<'ws>> {
    paths_and_routines
        .iter()
        .copied()
        .flat_map(|(path, features)| {
            features
                .into_iter()
                .map(|ft| ft.name())
                .filter_map(|ft_name| {
                    FixRoutine::try_new(workspace, path, ft_name)
                        .inspect_err(|e| {
                            eprintln!("fails to make FixRoutine object with error: {:#?}", e)
                        })
                        .ok()
                })
        })
        .collect()
}

async fn fixes<'ws>(
    generators: &'ws Generators,
    routine_fixers: &'ws mut Vec<FixRoutine<'ws>>,
) -> Vec<(&'ws Path, &'ws str, Option<&'ws String>)> {
    let mut fixes = Vec::new();
    for fix_cursor in routine_fixers {
        println!("fix routine");
        if let Some(_) = fix_cursor
            .side_effect(generators)
            .await
            .inspect_err(|e| {
                eprintln!(
                    "fails to execute side effects of the fixing routine cursor with error: {:#?}",
                    e
                )
            })
            .ok()
        {
            fixes.push((
                fix_cursor.path(),
                fix_cursor.feature().name(),
                fix_cursor.fixed_routine_body().clone(),
            ));
        } else {
            eprintln!("fails to wait for fix.")
        }
    }
    fixes
}

fn print_fixes(fixes: Vec<(&Path, &str, Option<&String>)>) {
    for (path, feature_name, maybe_fixed_body) in fixes {
        println!(
            "path: {:#?}\tfeature name: {:#?}\tfixed body: {:#?}",
            path, feature_name, maybe_fixed_body
        );
    }
}
