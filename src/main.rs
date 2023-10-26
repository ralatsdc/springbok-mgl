use clap::Parser;
use indexmap::IndexMap;
use log::{info, warn};
use scraper::{Html, Selector};
use url::Url;

// See:
// - https://docs.rs/reqwest/latest/reqwest/blocking/index.html
// - https://docs.rs/indexmap/latest/indexmap/index.html
// - https://docs.rs/scraper/0.17.1/scraper/#
// - https://docs.rs/url/latest/url/#
// - https://docs.rs/url/latest/url/struct.Url.html#

use springbok_mgl::Cli;

struct Entry {
    label_text: String,
    entry_token: String,
}

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

    let mut refiner_map = IndexMap::new();

    let refiner_selector = Selector::parse("div#refiners fieldset").unwrap();
    // Note that removing the "top" class will collect all hidden groups, some of which will be duplicates
    let group_selector = Selector::parse("div.refinerGroup.top").unwrap();
    let legend_selector = Selector::parse("legend").unwrap();
    let label_selector = Selector::parse("label").unwrap();
    let input_selector = Selector::parse("input").unwrap();
    for refiner_element in document.select(&refiner_selector) {
        let legend_element = refiner_element.select(&legend_selector).next().unwrap();
        let legend_text = legend_element.text().collect::<Vec<_>>()[0].trim();
        println!("\nLegend text: {:?}", legend_text);

        let mut group_map = IndexMap::new();

        let group_element = refiner_element.select(&group_selector).next().unwrap();
        for label_element in group_element.select(&label_selector) {
            let label_text = label_element.text().collect::<Vec<_>>()[1].trim();
            println!("Label text: {:?}", label_text);

            let entry_text: Vec<&str> = label_text.split(' ').collect();
            println!("Entry: {:}", entry_text[0]);

            let input_element = label_element.select(&input_selector).next().unwrap();
            let entry_token = input_element.value().attr("data-refinertoken").unwrap();
            println!("Entry token: {}", entry_token);

            group_map.insert(
                String::from(entry_text[0]),
                Entry {
                    label_text: String::from(label_text),
                    entry_token: String::from(entry_token),
                },
            );
        }

        refiner_map.insert(String::from(legend_text), group_map);
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
