use clap::Parser;
use eiffel_tools_lib::language_server_protocol::commands::modify_in_place;
use eiffel_tools_lib::tracing::info;
use std::path::Path;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(long)]
    class_paths_file: std::path::PathBuf,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    remove_comments(Args::parse()).await;

    info!("DONE FIXING CLASSES.");
}

async fn remove_comments(Args { class_paths_file }: Args) {
    let class_paths = class_paths(&class_paths_file).await;

    for path in class_paths {
        modify_in_place::clear_comments(&path).await;
        println!("Removed comments from {path:#?}");
    }
}

async fn class_paths(file_listing_paths: &Path) -> Vec<PathBuf> {
    tokio::fs::read(file_listing_paths)
        .await
        .inspect_err(|e| eprintln!("Fails reading {file_listing_paths:#?} with {e:#?}"))
        .ok()
        .and_then(|text| {
            String::from_utf8(text)
                .inspect_err(|e| {
                    eprintln!("Fails converting content of {file_listing_paths:#?} to UFT-8 with {e:#?}.")
                })
                .ok()
        })
        .map(|content| {
            content
                .lines()
                .map(|class_path| PathBuf::from(class_path))
                .filter(|path|
                    {let is_eiffel_extension = path.extension().is_some_and(|ext| ext == "e");
                        if !is_eiffel_extension{
                            eprintln!("Found {path:#?} in the class_path_file, but only the eiffel `e` extension is allowed.");
                        }
                        is_eiffel_extension
                    }
                ).inspect(|path| println!("Removing commments from file {path:#?}")).collect()
        })
        .unwrap_or_default()
}
