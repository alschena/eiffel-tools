use clap::Parser;
use eiffel_tools_lib::config::System;
use eiffel_tools_lib::workspace::Workspace;
use std::sync::Arc;
use tokio::sync::RwLock;

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    config_file: std::path::PathBuf,
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let args = Args::parse();
    let config_file = args.config_file;

    let system = match config_file.extension() {
        Some(ext) if ext == "ecf" => System::parse_from_file(&config_file).unwrap_or_else(|| {
            panic!("fails to parse eiffel system from ecf: {:#?}", &config_file)
        }),
        _ => panic!("the config file must be an eiffel `ecf` file"),
    };

    let ws = Arc::new(RwLock::new(Workspace::default()));

    let parsing_handle = tokio::spawn(async move {
        let mut ws = ws.write().await;
        ws.load_system(&system).await;
    });

    let _ = parsing_handle
        .await
        .inspect_err(|e| eprintln!("awaiting parsing fails with:{:#?} ", e));

    println!("DONE.");
}
