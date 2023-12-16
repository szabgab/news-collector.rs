use clap::Parser;
use serde::Deserialize;

const FEEDS: &str = "feeds";

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[arg(long, default_value_t = false)]
    download: bool,
}

#[derive(Debug, Deserialize)]
struct Feed {
    site: String,
    url: String,
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
        download(&config);
    }
}

fn download(config: &Config) {
    let feeds_folder = std::path::PathBuf::from(FEEDS);
    if !feeds_folder.exists() {
        match std::fs::create_dir(feeds_folder) {
            Ok(_) => {}
            Err(err) => {
                log::error!("Could not create the '{}' folder: {}", FEEDS, err);
                std::process::exit(1);
            }
        }
    }

    for feed in &config.feeds {
        log::info!("{} {} {}", feed.title, feed.site, feed.url);

        let res = match reqwest::blocking::get(&feed.url) {
            Ok(res) => res,
            Err(err) => {
                log::error!("Error while fetching {}: {}", feed.url, err);
                continue;
            }
        };

        log::info!("status: {:?}", res.status());
        if res.status() == 200 {
            println!("saving!");
            let text = match res.text() {
                Ok(val) => val,
                Err(err) => {
                    log::error!("Error: {}", err);
                    continue;
                }
            };
            log::debug!("text: {}", text);
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
