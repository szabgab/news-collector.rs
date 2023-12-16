# Collecting news from atom and RSS feeds



## Design

Separate commands for

* Download the feeds and store them locally on the file system.
* Collect the data from the locally stored feeds and store in some local database (e.g. json files) (these need to be kept between runs)
* Generate the web site.
* Send email with all the entries that were published in the last N hours. (We can schedule one to run once and hour with --hours 1 and one scheduled once a day with --hours 24)


## Configuration

* List of feeds
* How much time to display on the web site? (posts that were published in the last 26 hours?).
* How much time to keep the old entries? (posts that were published in the last 48 hours?).


## Deployment

GitHub Actions
We could add the "locally stored json files" to the web site and then we could download them from there before every run.

Also store the log on the web server so we can see it there as well.

