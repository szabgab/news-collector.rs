use chrono::{DateTime, Utc};
use clap::Parser;
use feed_rs::parser;
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize, Serializer};
use std::fs::File;
use std::io::Write;

const FEEDS: &str = "feeds";
const SITE: &str = "_site";

#[derive(Parser)]
#[command(version)]
struct Cli {
    #[arg(long, default_value_t = false)]
    download: bool,

    #[arg(long, default_value_t = 0)]
    limit: u32,

    #[arg(long)]
    config: String,

    #[arg(long, default_value_t = false)]
    web: bool,
}

#[derive(Debug, Deserialize)]
struct FeedConfig {
    site: String,
    url: String,
    title: String,
}

#[derive(Debug, Deserialize)]
struct Config {
    title: String,
    description: String,
    feeds: Vec<FeedConfig>,
}

#[derive(Debug, Serialize)]
struct Post {
    title: String,
    url: String,

    #[serde(serialize_with = "ts_iso")]
    updated: DateTime<Utc>,
    site_title: String,
    feed_id: String,
}

fn ts_iso<S>(date: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let string = date.format("%Y-%m-%d %H:%M:%S").to_string();
    s.serialize_str(&string)
}

fn main() {
    simple_logger::init_with_env().unwrap();
    log::info!("Starting the News collector");

    let args = Cli::parse();
    let config = read_config(&args.config);
    log::debug!("{:?}", config);

    if args.download {
        download(&config, args.limit);
    }

    if args.web {
        generate_web_page(&config);
    }
}

fn read_feeds(config: &Config) -> Vec<Post> {
    log::info!("Start reading feeds");

    let feeds_folder = std::path::PathBuf::from(FEEDS);
    if !feeds_folder.exists() {
        log::error!("Feed folder {} does not exist. Exciting.", FEEDS);
        std::process::exit(1);
    }
    let mut posts: Vec<Post> = vec![];

    for feed in &config.feeds {
        log::info!("{} {} {}", feed.title, feed.site, feed.url);
        let filename = get_filename(feed);
        log::info!("file: {:?}", filename);
        if !filename.exists() {
            log::warn!("File {:?} does not exist", filename);
            continue;
        }

        let site_title = feed.title.clone();
        // let site_title = match feed.title {
        //     Some(val) => String::from("XX"), //format!("{}", val),
        //     None => {
        //         log::error!("Title is missing from the configuration");
        //         continue;
        //     }
        // };

        let text = std::fs::read_to_string(&filename).unwrap();
        let feed = match parser::parse(text.as_bytes()) {
            Ok(val) => val,
            Err(err) => {
                log::error!("feed: {:?} error {}", feed, err);
                continue;
            }
        };
        //log::debug!("feed: {:?}", feed);
        for entry in feed.entries {
            //log::debug!("title: {:?}", entry.title);
            //log::debug!("updated: {:?}", entry.updated);
            //log::debug!("links: {:?}", entry.links);
            let title = match entry.title {
                Some(val) => val.content,
                None => {
                    log::warn!("Missing title");
                    continue;
                }
            };
            let updated = match entry.updated {
                Some(val) => val,
                None => {
                    log::warn!("Missing updated field");
                    continue;
                }
            };

            posts.push(Post {
                title,
                updated,
                url: entry.links[0].href.clone(), // TODO why is this a list?
                feed_id: filename.file_name().unwrap().to_str().unwrap().to_string(),
                site_title: site_title.clone(),
            });
        }
    }

    posts.sort_by(|a, b| b.updated.cmp(&a.updated));
    posts
}

fn generate_web_page(config: &Config) {
    log::info!("Start generating web page");

    let now: DateTime<Utc> = Utc::now();

    let posts = read_feeds(config);
    for post in &posts {
        log::debug!("{}", post.title);
    }

    let site_folder = std::path::PathBuf::from(SITE);
    if !site_folder.exists() {
        match std::fs::create_dir(&site_folder) {
            Ok(_) => {}
            Err(err) => {
                log::error!("Could not create the '{}' folder: {}", SITE, err);
                std::process::exit(1);
            }
        }
    }

    let template = include_str!("../templates/index.html");
    let template = liquid::ParserBuilder::with_stdlib()
        .build()
        .unwrap()
        .parse(template)
        .unwrap();

    let globals = liquid::object!({
        "posts": &posts,
        "title": config.title,
        "description": config.description,
        "now": now,
    });
    let output = template.render(&globals).unwrap();

    let path = site_folder.join("index.html");
    let mut file = File::create(path).unwrap();
    writeln!(&mut file, "{}", output).unwrap();
}

fn download(config: &Config, limit: u32) {
    log::info!("Start downloading feeds");

    let feeds_folder = std::path::PathBuf::from(FEEDS);
    if !feeds_folder.exists() {
        match std::fs::create_dir(&feeds_folder) {
            Ok(_) => {}
            Err(err) => {
                log::error!("Could not create the '{}' folder: {}", FEEDS, err);
                std::process::exit(1);
            }
        }
    }

    let client = reqwest::blocking::Client::new();

    let mut count = 0;
    for feed in &config.feeds {
        log::info!("{} {} {}", feed.title, feed.site, feed.url);

        let res = match client
            .get(&feed.url)
            .header(USER_AGENT, "News Collector 0.1.0")
            .send()
        {
            Ok(res) => res,
            Err(err) => {
                log::error!("Error while fetching {}: {}", feed.url, err);
                continue;
            }
        };

        if res.status() != 200 {
            log::error!("status was {:?} when fetching {}", res.status(), feed.url);
            continue;
        }

        let filename = get_filename(feed);

        log::info!("Saving feed as '{:?}'", filename);
        let text = match res.text() {
            Ok(val) => val,
            Err(err) => {
                log::error!("Error: {}", err);
                continue;
            }
        };
        let mut file = File::create(filename).unwrap();
        writeln!(&mut file, "{}", &text).unwrap();

        count += 1;
        if 0 < limit && limit <= count {
            break;
        }
    }
}

fn get_filename(feed: &FeedConfig) -> std::path::PathBuf {
    let feeds_folder = std::path::PathBuf::from(FEEDS);
    let filename = feed.url.replace("://", "-").replace('/', "-");
    feeds_folder.join(filename)
}

fn read_config(path: &str) -> Config {
    let yaml_string = std::fs::read_to_string(path).unwrap();
    let cfg: Config = serde_yaml::from_str(&yaml_string).unwrap_or_else(|err| {
        eprintln!("Could not read YAML config file '{path}': {err}");
        std::process::exit(1);
    });
    cfg
}
