use serde::Deserialize;

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
    let config = read_config("rust.yaml");
    println!("{:?}", config);
    for feed in config.feeds {
        println!("{} {} {}", feed.title, feed.site, feed.feed);
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
