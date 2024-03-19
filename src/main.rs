use chrono::{DateTime, SubsecRound, Utc};
use clap::Parser;
use feed_rs::parser;
use regex::Regex;
use reqwest::header::USER_AGENT;
use serde::{Deserialize, Serialize, Serializer};
use std::fs::File;
use std::io::Write;
//use std::ops::ControlFlow;

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

#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct FeedConfig {
    site: String,
    url: String,
    title: String,

    #[serde(default = "get_empty_string")]
    filter: String,

    #[serde(default = "get_empty_string")]
    feed_id: String,
}

fn get_empty_string() -> String {
    String::new()
}

#[allow(clippy::struct_field_names)]
#[derive(Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Config {
    title: String,
    description: String,
    feeds: Vec<FeedConfig>,
    per_feed_limit: Option<usize>,
    config_url: Option<String>,
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

    for feed_cfg in &config.feeds {
        log::info!(
            "Feed title='{}' site='{}' url='{}'",
            feed_cfg.title,
            feed_cfg.site,
            feed_cfg.url
        );
        let filename = get_filename(feed_cfg);
        log::info!("file: {filename:?}");
        if !filename.exists() {
            log::warn!("File {filename:?} does not exist");
            continue;
        }

        let text = std::fs::read_to_string(&filename).unwrap();
        let feed = match parser::parse(text.as_bytes()) {
            Ok(val) => val,
            Err(err) => {
                log::error!("Parsing feed: {feed_cfg:?} error {err}");
                continue;
            }
        };

        posts.append(&mut get_posts(feed, feed_cfg, config));
    }

    #[allow(clippy::min_ident_chars)]
    posts.sort_by(|a, b| b.published.cmp(&a.published));

    for post in &posts {
        log::debug!("{}", post.title);
    }

    Ok(posts)
}

fn get_posts(feed: feed_rs::model::Feed, feed_cfg: &FeedConfig, config: &Config) -> Vec<Post> {
    let mut my_posts: Vec<Post> = vec![];

    let mut per_feed_counter: usize = 0;
    //log::debug!("feed: {feed:?}");
    for entry in feed.entries {
        let Some(post) = get_post(entry, &feed_cfg.filter, &feed_cfg.feed_id, &feed_cfg.title)
        else {
            continue;
        };
        my_posts.push(post);

        if let Some(per_feed_limit) = config.per_feed_limit {
            per_feed_counter = per_feed_counter.saturating_add(1);
            if per_feed_limit <= per_feed_counter {
                break;
            }
        };
    }
    my_posts
}

fn get_post(
    entry: feed_rs::model::Entry,
    filter: &String,
    feed_id: &str,
    site_title: &str,
) -> Option<Post> {
    let Some(published) = entry.published else {
        log::error!("Missing published field {:?}", entry);
        return None;
    };
    let Some(link) = entry.links.first() else {
        log::error!("No link found {:?}", entry);
        return None;
    };
    let Some(title) = entry.title else {
        log::error!("Missing title {:?}", &entry);
        return None;
    };
    let title = title.content.clone();
    if !filter.is_empty() {
        let re = match Regex::new(filter) {
            Ok(re) => re,
            Err(err) => {
                log::error!("filter '{filter}' is not a valid regex: {err}");
                return None;
            }
        };

        let summary = match entry.summary {
            Some(val) => val.content,
            None => String::new(),
        };

        if re.captures(title.to_lowercase().as_str()).is_none()
            && re.captures(summary.to_lowercase().as_str()).is_none()
        {
            log::info!("Skipping entry {title} as it did not match filter '{filter}'");
            return None;
        }
        log::info!("Including entry {title} as it matched filter '{filter}'");
    }
    let post = Post {
        title,
        published,
        url: link.href.clone(), // TODO why is this a list?
        feed_id: feed_id.to_owned(),
        site_title: site_title.to_owned(),
    };
    Some(post)
}

fn generate_web_page(config: &Config) -> Result<(), String> {
    log::info!("Start generating web page");

    let now: DateTime<Utc> = Utc::now().trunc_subsecs(0);

    let mut partials = Partials::empty();
    partials.add(
        "templates/navbar.html",
        include_str!("../templates/navbar.html"),
    );
    partials.add(
        "templates/footer.html",
        include_str!("../templates/footer.html"),
    );

    let posts = read_feeds(config)?;

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
        "config": &config,
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
    let mut cfg: Config = match serde_yaml::from_str(&yaml_string) {
        Ok(val) => val,
        Err(err) => return Err(format!("Could not read YAML config file '{path}': {err}")),
    };

    cfg.feeds = cfg
        .feeds
        .into_iter()
        .map(|mut feed| {
            feed.feed_id = get_filename(&feed)
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .to_owned();
            feed
        })
        .collect::<Vec<FeedConfig>>();

    Ok(cfg)
}
