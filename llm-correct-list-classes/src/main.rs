use clap::Parser;
use eiffel_tools_lib::code_entities::prelude::*;
use eiffel_tools_lib::config::System;
use eiffel_tools_lib::generators::Generators;
use eiffel_tools_lib::language_server_protocol::commands::Command;
use eiffel_tools_lib::language_server_protocol::commands::FixRoutine;
use eiffel_tools_lib::workspace::Workspace;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config_file: std::path::PathBuf,
    #[arg(short, long)]
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

    let classes_names = tokio::spawn(async move {
        tokio::fs::read(classes_file)
            .await
            .inspect_err(|e| eprintln!("fails to read classes_file with error: {:#?}",e))
            .ok()
            .and_then(|text| {
                String::from_utf8(text)
                    .inspect_err(|e| {
                        eprintln!("fails to convert content of classes file to UFT8 string with error: {:#?}", e)
                    })
                    .ok()
            })
            .map(|text| {
                text.lines()
                    .map(|name| ClassName(name.to_string()))
                    .collect::<Vec<_>>()
            })
    })
    .await
    .inspect_err(|e| {
        eprintln!(
            "fails to extract classes from classes_file with error: {:#?}",
            e
        )
    }).ok().flatten();

    let ws = workspace.clone();
    let maybe_fix_classes_handle = classes_names.map(|classes_names| {
        tokio::spawn(async move {
            let generators = Generators::default();
            // Add generators here

            let ws = ws.read().await;

            let mut paths = Vec::new();
            let mut feature_for_path = Vec::new();
            for name in classes_names {
                let path = ws.path(&name);
                paths.push(path);
                feature_for_path.push(ws.class(path).map(|class| class.features()));
            }

            let mut fix_routine = Vec::new();
            for (num, path) in paths.into_iter().enumerate() {
                let features = feature_for_path[num];
                if let Some(features) = features {
                    for feature_name in features {
                        if let Some(fix_routine_cursor) =
                            FixRoutine::try_new(&ws, path, feature_name.name())
                                .inspect_err(|e| {
                                    eprintln!(
                                        "fails to construct routine fixer with error: {:#?}",
                                        e
                                    )
                                })
                                .ok()
                        {
                            fix_routine.push(fix_routine_cursor)
                        }
                    }
                }
            }

            let mut outcomes = Vec::new();
            for mut fix_cursor in fix_routine {
                let _ = fix_cursor
                    .side_effect(&generators)
                    .await
                    .inspect_err(|e| {
                        eprintln!("fails to execute side effects of the fixing routine cursor with error: {:#?}",e)
                    })
                    .ok();

                outcomes.push((fix_cursor.path.clone(), fix_cursor.feature.name(), fix_cursor.fixed_routine_body))
            }

            for (path, feature_name, maybe_fixed_body) in outcomes {
                println!("path: {:#?}\tfeature name: {:#?}\tfixed body: {:#?}",path,feature_name,maybe_fixed_body);
                
            }
        })
    });

    if let Some(fix_classes_handle) = maybe_fix_classes_handle {
        let _ = fix_classes_handle.await.inspect_err(|e|eprintln!("fails waiting for classes fixer with error: {:#?}",e)).ok();
        
    }

    println!("DONE FIXING CLASSES.");
}
