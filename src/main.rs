use chrono::{DateTime, SubsecRound, Utc};
use clap::Parser;
use feed_rs::parser;
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize, Serializer};
use std::fs::File;
use std::io::Write;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const FEEDS: &str = "feeds";
const SITE: &str = "_site";

pub type Partials = liquid::partials::EagerCompiler<liquid::partials::InMemorySource>;

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
    published: DateTime<Utc>,
    site_title: String,
    feed_id: String,
}

#[allow(clippy::min_ident_chars)]
fn ts_iso<S>(date: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let string = date.format("%Y-%m-%d %H:%M:%S").to_string();
    s.serialize_str(&string)
}

fn main() {
    simple_logger::init_with_env().unwrap();
    log::info!("Starting the News collector version {VERSION}");

    match run() {
        Ok(()) => {}
        Err(err) => {
            log::error!("{err}");
            std::process::exit(1);
        }
    };
    log::info!("Ending the News collector");
}
fn run() -> Result<(), String> {
    let args = Cli::parse();
    let config = read_config(&args.config)?;

    log::debug!("{config:?}");

    if args.download {
        let count = download(&config, args.limit)?;
        log::info!("Downloaded: {count} feeds out of {}", config.feeds.len());
    }

    if args.web {
        generate_web_page(&config)?;
    }
    Ok(())
}

fn read_feeds(config: &Config) -> Result<Vec<Post>, String> {
    log::info!("Start reading feeds");

    let feeds_folder = std::path::PathBuf::from(FEEDS);
    if !feeds_folder.exists() {
        return Err(format!("Feed folder {FEEDS} does not exist. Exciting."));
    }
    let mut posts: Vec<Post> = vec![];

    for feed in &config.feeds {
        log::info!(
            "Feed title='{}' site='{}' url='{}'",
            feed.title,
            feed.site,
            feed.url
        );
        let filename = get_filename(feed);
        log::info!("file: {filename:?}");
        if !filename.exists() {
            log::warn!("File {filename:?} does not exist");
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
                log::error!("Parsing feed: {feed:?} error {err}");
                continue;
            }
        };
        //log::debug!("feed: {feed:?}");
        for entry in feed.entries {
            // let title = match &entry.title {
            //     Some(val) => val.content.clone(),
            //     None => {
            //         log::warn!("Missing title {:?}", &entry);
            //         continue;
            //     }
            // };

            let Some(published) = entry.published else {
                log::error!("Missing published field {:?}", entry);
                continue;
            };

            let Some(link) = entry.links.first() else {
                log::error!("No link found {:?}", entry);
                continue;
            };

            let Some(title) = entry.title else {
                log::error!("Missing title {:?}", &entry);
                continue;
            };
            let title = title.content.clone();

            posts.push(Post {
                title,
                published,
                url: link.href.clone(), // TODO why is this a list?
                feed_id: filename.file_name().unwrap().to_str().unwrap().to_owned(),
                site_title: site_title.clone(),
            });
        }
    }

    #[allow(clippy::min_ident_chars)]
    posts.sort_by(|a, b| b.published.cmp(&a.published));
    Ok(posts)
}

fn generate_web_page(config: &Config) -> Result<(), String> {
    log::info!("Start generating web page");

    let now: DateTime<Utc> = Utc::now().trunc_subsecs(0);

    let mut partials = Partials::empty();
    partials.add(
        "templates/navbar.html",
        include_str!("../templates/navbar.html"),
    );

    let posts = read_feeds(config)?;
    for post in &posts {
        log::debug!("{}", post.title);
    }

    let site_folder = std::path::PathBuf::from(SITE);
    if !site_folder.exists() {
        match std::fs::create_dir_all(&site_folder) {
            Ok(()) => {}
            Err(err) => return Err(format!("Could not create the '{SITE}' folder: {err}")),
        }
    }

    let template = include_str!("../templates/index.html");
    let template = liquid::ParserBuilder::with_stdlib()
        .partials(partials)
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
    writeln!(&mut file, "{output}").unwrap();
    Ok(())
}

fn download(config: &Config, limit: u32) -> Result<u32, String> {
    log::info!("Start downloading feeds");

    let feeds_folder = std::path::PathBuf::from(FEEDS);
    if !feeds_folder.exists() {
        match std::fs::create_dir_all(&feeds_folder) {
            Ok(()) => {}
            Err(err) => return Err(format!("Could not create the '{FEEDS}' folder: {err}")),
        }
    }

    let client = reqwest::blocking::Client::new();

    let mut count: u32 = 0;
    for feed in &config.feeds {
        log::info!("{} {} {}", feed.title, feed.site, feed.url);

        let res = match client
            .get(&feed.url)
            .header(USER_AGENT, format!("News Collector {VERSION}"))
            .send()
        {
            Ok(res) => res,
            Err(err) => {
                log::error!("Error while fetching {}: {err}", feed.url);
                continue;
            }
        };

        if res.status() != 200 {
            log::error!("status was {:?} when fetching {}", res.status(), feed.url);
            continue;
        }

        let filename = get_filename(feed);

        log::info!("Saving feed as '{filename:?}'");
        let text = match res.text() {
            Ok(val) => val,
            Err(err) => {
                log::error!("Error: {err}");
                continue;
            }
        };
        let mut file = File::create(filename).unwrap();
        writeln!(&mut file, "{}", &text).unwrap();

        count = count.saturating_add(1); // Making clippy::arithmetic_side_effects happy.
        if 0 < limit && limit <= count {
            break;
        }
    }
    Ok(count)
}

fn get_filename(feed: &FeedConfig) -> std::path::PathBuf {
    let feeds_folder = std::path::PathBuf::from(FEEDS);
    let filename = feed.url.replace("://", "-").replace('/', "-");
    feeds_folder.join(filename)
}

fn read_config(path: &str) -> Result<Config, String> {
    let yaml_string = match std::fs::read_to_string(path) {
        Ok(val) => val,
        Err(err) => return Err(format!("Config file '{path}' could not be read {err}")),
    };
    let cfg: Config = match serde_yaml::from_str(&yaml_string) {
        Ok(val) => val,
        Err(err) => return Err(format!("Could not read YAML config file '{path}': {err}")),
    };
    Ok(cfg)
}
