use clap::Parser;
use serde::Deserialize;

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[arg(long, default_value_t = false)]
    download: bool,
}

#[derive(Debug, Deserialize)]
struct Feed {
    site: String,
    feed: String,
    title: String,
}

#[derive(Debug, Deserialize)]
struct Config {
    feeds: Vec<Feed>,
}

fn main() {
    simple_logger::init_with_env().unwrap();
    log::info!("Starting the News collector");

    let args = Cli::parse();
    let config = read_config("rust.yaml");
    log::debug!("{:?}", config);

    if args.download {
        for feed in config.feeds {
            log::info!("{} {} {}", feed.title, feed.site, feed.feed);
        }
    }
}

fn read_config(path: &str) -> Config {
    let yaml_string = std::fs::read_to_string(path).unwrap();
    let cfg: Config = serde_yaml::from_str(&yaml_string).unwrap_or_else(|err| {
        eprintln!("Could not read YAML config file '{path}': {err}");
        std::process::exit(1);
    });
    cfg
}
