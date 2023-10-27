use clap::Parser;
use indexmap::IndexMap;
use log::debug;
use scraper::{Html, Selector};
use url::Url;

// See:
// - https://docs.rs/clap/latest/clap/_derive/_tutorial/chapter_0/index.html#
// - https://docs.rs/clap/latest/clap/struct.Arg.html#method.default_missing_value
// - https://docs.rs/reqwest/latest/reqwest/blocking/index.html
// - https://docs.rs/indexmap/latest/indexmap/index.html
// - https://docs.rs/scraper/0.17.1/scraper/#
// - https://docs.rs/url/latest/url/#
// - https://docs.rs/url/latest/url/struct.Url.html#

/// Produce strikethrough and underline markup for a bill before the Massachusetts legislature
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Search for legislation using this search term
    #[arg(short = 's', long)]
    pub search_term: Option<String>,

    /// Identify legislation from this legislative session
    #[arg(short = 'c', long, default_value = "193rd", num_args = 0..=1, default_missing_value = "MISSING")]
    pub general_court: Option<String>,

    /// Include legislation in this branch of the legislature
    #[arg(short = 'b', long, num_args = 0..=1, default_missing_value = "MISSING")]
    pub branch: Option<String>,

    /// Include legislation sponsored by this legislator
    #[arg(short = 'l', long, num_args = 0..=1, default_missing_value = "MISSING")]
    pub sponsor_legislator: Option<String>,

    /// Include legislation sponsored by this legislative committee
    #[arg(short = 'm', long, num_args = 0..=1, default_missing_value = "MISSING")]
    pub sponsor_committee: Option<String>,

    /// Include legislation sponsored by a governor, or state organization
    #[arg(short = 'o', long, num_args = 0..=1, default_missing_value = "MISSING")]
    pub sponsor_other: Option<String>,

    /// Identify legislation of this document type
    #[arg(short = 'd', long, num_args = 0..=1, default_missing_value = "MISSING")]
    pub document_type: Option<String>,
}

pub fn create_refiner_map(refiner_map: &mut IndexMap<String, IndexMap<String, Entry>>) {
    let body = reqwest::blocking::get("https://malegislature.gov/Bills/Search?SearchTerms=&Page=1")
        .unwrap()
        .text()
        .unwrap();
    let document = Html::parse_document(body.as_str());

    let refiner_selector = Selector::parse("div#refiners fieldset").unwrap();
    // Note that removing the "top" class will collect all hidden groups, some of which will be duplicates
    let group_selector = Selector::parse("div.refinerGroup.top").unwrap();
    let legend_selector = Selector::parse("legend").unwrap();
    let label_selector = Selector::parse("label").unwrap();
    let input_selector = Selector::parse("input").unwrap();
    for refiner_element in document.select(&refiner_selector) {
        let legend_element = refiner_element.select(&legend_selector).next().unwrap();
        let legend_text = legend_element.text().collect::<Vec<_>>()[0].trim();
        debug!("\nLegend text: {:?}", legend_text);

        let mut group_map = IndexMap::new();

        let group_element = refiner_element.select(&group_selector).next().unwrap();
        for label_element in group_element.select(&label_selector) {
            let label_text = label_element.text().collect::<Vec<_>>()[1].trim();
            debug!("\nLabel text: {:?}", label_text);

            let entry_text: Vec<&str> = label_text.split(' ').collect();
            // TODO: Make this value unique for all refiners
            debug!("Entry: {:}", entry_text[0]);

            let input_element = label_element.select(&input_selector).next().unwrap();
            let entry_token = input_element.value().attr("data-refinertoken").unwrap();
            debug!("Entry token: {}", entry_token);

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
}

pub struct Entry {
    pub label_text: String,
    pub entry_token: String,
}

pub fn list_entries_or_append_query_pair(
    argument: &Option<String>,
    map: &IndexMap<String, Entry>,
    field: &mut String,
    search_url: &mut Url,
) {
    match argument {
        Some(value) if value == &String::from("MISSING") => {
            for (key, entry) in map.iter() {
                println!(r#"Use "{}" for "{}""#, key, entry.label_text);
            }
        }
        Some(value) => {
            search_url
                .query_pairs_mut()
                .append_pair(field, map.get(value).unwrap().entry_token.as_str());
        }
        None => (),
    }
}
