use clap::Parser;
use log::{info, warn};
use scraper::{Html, Selector};
use url::Url;

// See:
// - https://docs.rs/reqwest/latest/reqwest/blocking/index.html
// - https://docs.rs/scraper/0.17.1/scraper/#
// - https://docs.rs/url/latest/url/#
// - https://docs.rs/url/latest/url/struct.Url.html#

use springbok_mgl::Cli;

fn main() {
    env_logger::init();
    info!("starting up");
    warn!("oops, nothing implemented!");

    // Demonstrate getting and parsing a page to get needed search tokens

    let body = reqwest::blocking::get("https://malegislature.gov/Bills/Search?SearchTerms=&Page=1")
        .unwrap()
        .text()
        .unwrap();
    let document = Html::parse_document(body.as_str());
    let group_selector = Selector::parse("div.refinerGroup.top").unwrap();
    let label_selector = Selector::parse("label").unwrap();
    let input_selector = Selector::parse("input").unwrap();
    for group in document.select(&group_selector) {
        println!(
            "Group name: {}\n",
            group.value().attr("data-refinername").unwrap()
        );
        for label in group.select(&label_selector) {
            println!(
                "Label text: {:?}\n",
                label.text().collect::<Vec<_>>()[1].trim()
            );
            for input in label.select(&input_selector) {
                println!(
                    "Input token: {}\n",
                    input.value().attr("data-refinertoken").unwrap()
                );
            }
        }
    }

    // Demonstrate parsing and handling of command line arguments

    let cli = Cli::parse();

    let mut search_url = Url::parse("https://malegislature.gov/Bills/Search").unwrap();

    // https://malegislature.gov/Bills/Search?SearchTerms=mbta
    if let Some(search_term) = cli.search_term {
        println!("Value for search term: {search_term}");
        search_url.set_query(Some(format!("SearchTerms={search_term}").as_str()));
    }

    println!("Value for search URL: {search_url}");
}
