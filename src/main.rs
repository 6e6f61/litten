use clap::{Parser, Subcommand};
use log::{error, info};
use tokio::task::JoinSet;

mod configuration;
mod http;
use configuration::Configuration;

#[derive(Debug, Parser)]
#[command(author, version, about, long_about = None)]
struct Arguments {
    #[arg(long, short, default_value = "/etc/litten.toml")]
    config: String,
    #[command(subcommand)]
    subcommands: Option<Subcommands>,
}

#[derive(Debug, Subcommand)]
enum Subcommands {
    Live {
        #[arg(long, short, default_value = "11734")]
        port: u16,
        #[arg(long, short, default_value = ".")]
        root: String,
    },
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Arguments::parse();
    env_logger::init();

    let conf_file = std::fs::read_to_string(args.config)?;
    let configuration: Configuration = match toml::from_str(&conf_file) {
        Ok(v) => v,
        Err(e) => {
            error!("{}", e);
            return Ok(());
        }
    };
    info!("parsed configuration ok");

    let mut services = JoinSet::new();

    if let Some(http) = configuration.http {
        services.spawn(async move { http.serve().await });
    }

    while let Some(svc) = services.join_next().await {
        svc??;
    }

    Ok(())
}
