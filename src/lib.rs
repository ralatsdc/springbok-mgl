use clap::Parser;
use indexmap::IndexMap;
use log::debug;
use scraper::{ElementRef, Html, Selector};
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

pub fn create_refiner_map() -> IndexMap<String, IndexMap<String, Entry>> {
    // Use an IndexMap to preserve order
    let mut refiner_map = IndexMap::new();

    // Get the page from which to parse refiners
    let body = reqwest::blocking::get("https://malegislature.gov/Bills/Search?SearchTerms=&Page=1")
        .unwrap()
        .text()
        .unwrap();
    let document = Html::parse_document(body.as_str());

    // Define all selectors required to find the refiners
    let refiner_selector = Selector::parse("div#refiners").unwrap();
    let group_selector = Selector::parse("fieldset").unwrap();
    let title_selector = Selector::parse("h4.modal-title").unwrap();
    let legend_selector = Selector::parse("legend").unwrap();
    let body_selector = Selector::parse("div.modal-body").unwrap();
    let label_selector = Selector::parse("label").unwrap();
    let input_selector = Selector::parse("input").unwrap();

    // Find the div#refiners element which contains all refiner groups, then consider each group
    let refiner_element = document.select(&refiner_selector).next().unwrap();
    for group_element in refiner_element.select(&group_selector) {
        // Use an IndexMap to preserve order
        let mut group_map = IndexMap::new();

        // Find the title in the h4.modal-title element ...
        let group_label_element: ElementRef = match group_element.select(&title_selector).next() {
            None => {
                // ... but if it isn't there, find it in the fieldset element
                group_element.select(&legend_selector).next().unwrap()
            }
            Some(element) => element,
        };
        let group_label = String::from(group_label_element.text().collect::<Vec<_>>()[0].trim());
        debug!("\nLegend text: {:?}", group_label);

        // Find the div.modal-body element ...
        let group_column_element = match group_element.select(&body_selector).next() {
            None => {
                // ... but if it isn't there, clone the fieldset element, then consider each row label element
                group_element.clone()
            }
            Some(element) => element,
        };
        for row_label_element in group_column_element.select(&label_selector) {
            // Assign label for this entry
            let entry_label = row_label_element.text().collect::<Vec<_>>()[1]
                .trim()
                .replace("  ", " ");
            debug!("Label text: {:?}", entry_label);

            // Create a unique key from the label
            let mut entry_key: String =
                String::from(entry_label.split(' ').collect::<Vec<&str>>()[0])
                    .trim_end_matches(",")
                    .parse()
                    .unwrap();
            if !(group_label.contains("Court")
                || group_label.contains("Branch")
                || group_label.contains("Legislator"))
            {
                let words = entry_label.split(" ").collect::<Vec<&str>>();
                entry_key = words[..words.len() - 1]
                    .join("-")
                    .replace("/", "-")
                    .replace("(", "")
                    .replace(")", "")
                    .replace("'", "")
                    .replace(",", "")
                    .replace(".", "");
            }
            debug!("Entry: {:?}", entry_key);

            // Find the token for this entry
            let input_element = row_label_element.select(&input_selector).next().unwrap();
            let entry_token = input_element.value().attr("data-refinertoken").unwrap();
            debug!("Entry token: {}", entry_token);

            // Collect each group entry key, label, and token
            group_map.insert(
                String::from(entry_key),
                Entry {
                    label_text: String::from(entry_label),
                    entry_token: String::from(entry_token),
                },
            );
        }
        refiner_map.insert(String::from(group_label), group_map);
    }
    refiner_map
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
