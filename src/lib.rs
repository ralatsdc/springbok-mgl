use clap::Parser;
use indexmap::IndexMap;
use log::debug;
use regex::Regex;
use scraper::{ElementRef, Html, Selector};
use std::fs::File;
use std::io::Write;
use std::path::Path;
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
    /// List legislation for the current general court
    #[arg(short = 'l', long)]
    pub list: bool,

    /// Search for legislation using this search term
    #[arg(short = 's', long)]
    pub search_term: Option<String>,

    /// Identify legislation from this legislative session
    #[arg(short = 'C', long, default_value = "193rd", num_args = 0..=1, default_missing_value = "MISSING")]
    pub general_court: Option<String>,

    /// Include legislation in this branch of the legislature
    #[arg(short = 'B', long, num_args = 0..=1, default_missing_value = "MISSING")]
    pub branch: Option<String>,

    /// Include legislation sponsored by this legislator
    #[arg(short = 'L', long, num_args = 0..=1, default_missing_value = "MISSING")]
    pub sponsor_legislator: Option<String>,

    /// Include legislation sponsored by this legislative committee
    #[arg(short = 'M', long, num_args = 0..=1, default_missing_value = "MISSING")]
    pub sponsor_committee: Option<String>,

    /// Include legislation sponsored by a governor, or state organization
    #[arg(short = 'O', long, num_args = 0..=1, default_missing_value = "MISSING")]
    pub sponsor_other: Option<String>,

    /// Identify legislation of this document type
    #[arg(short = 'D', long, num_args = 0..=1, default_missing_value = "MISSING")]
    pub document_type: Option<String>,

    /// Download the text of a bill when searching with the bill number
    #[arg(short = 'd', long)]
    pub download: bool,

    /// Download text into this filename
    #[arg(short = 'o', long)]
    pub output_filename: Option<String>,
}

pub struct RefinerEntry {
    pub refiner_label: String,
    pub refiner_token: String,
}

pub fn create_refiner_map() -> IndexMap<String, IndexMap<String, RefinerEntry>> {
    // Use an IndexMap to preserve order
    let mut refiner_map = IndexMap::new();

    // Get the page from which to parse refiners
    let body = reqwest::blocking::get("https://malegislature.gov/Bills/Search?SearchTerms=&Page=1")
        .unwrap()
        .text()
        .unwrap();
    let document = Html::parse_document(body.as_str());

    // Define all selectors required to select the refiners
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
        let mut refiner_group_map = IndexMap::new();

        // Find the title in the h4.modal-title element ...
        let group_label_element: ElementRef = match group_element.select(&title_selector).next() {
            None => {
                // ... but if it isn't there, find it in the fieldset element
                group_element.select(&legend_selector).next().unwrap()
            }
            Some(element) => element,
        };
        let group_label = String::from(group_label_element.text().collect::<Vec<_>>()[0].trim());
        debug!("\nGroup label: {:?}", group_label);

        // Find the div.modal-body element ...
        let group_column_element = match group_element.select(&body_selector).next() {
            None => {
                // ... but if it isn't there, clone the fieldset element, then consider each row label element
                group_element.clone()
            }
            Some(element) => element,
        };
        for row_label_element in group_column_element.select(&label_selector) {
            // Assign label for this refiner
            let refiner_label = row_label_element.text().collect::<Vec<_>>()[1]
                .trim()
                .replace("  ", " ");
            debug!("Refiner label: {:?}", refiner_label);

            // Create a unique key from the label
            let mut refiner_key: String =
                String::from(refiner_label.split(' ').collect::<Vec<&str>>()[0])
                    .trim_end_matches(",")
                    .parse::<String>()
                    .unwrap()
                    .replace("'", "-");
            if !(group_label.contains("Court")
                || group_label.contains("Branch")
                || group_label.contains("Legislator"))
            {
                let words = refiner_label.split(" ").collect::<Vec<&str>>();
                refiner_key = words[..words.len() - 1]
                    .join("-")
                    .replace("/", "-")
                    .replace("(", "")
                    .replace(")", "")
                    .replace("'", "")
                    .replace(",", "")
                    .replace(".", "");
            }
            debug!("Refiner key: {:?}", refiner_key);

            // Find the token for this entry
            let input_element = row_label_element.select(&input_selector).next().unwrap();
            let refiner_token = input_element.value().attr("data-refinertoken").unwrap();
            debug!("Refiner token: {}", refiner_token);

            // Collect each refiner group entry key, label, and token
            refiner_group_map.insert(
                String::from(refiner_key),
                RefinerEntry {
                    refiner_label: String::from(refiner_label),
                    refiner_token: String::from(refiner_token),
                },
            );
        }
        refiner_map.insert(String::from(group_label), refiner_group_map);
    }
    refiner_map
}

pub fn print_entries_or_append_query_pair(
    argument: Option<&str>,
    refiner_group_map: &IndexMap<String, RefinerEntry>,
    refiner_field: &mut String,
    search_url: &mut Url,
) -> Option<bool> {
    match argument {
        Some(refiner_key) if refiner_key == &String::from("MISSING") => {
            // Refiner key is missing, so list all possible keys
            for (refiner_key, refiner_entry) in refiner_group_map.iter() {
                println!(
                    r#"Use "{}" for "{}""#,
                    refiner_key, refiner_entry.refiner_label
                );
            }
            None
        }
        Some(refiner_key) => {
            // Refiner key is not missing, so append the query pair
            search_url.query_pairs_mut().append_pair(
                refiner_field,
                refiner_group_map
                    .get(refiner_key)
                    .unwrap()
                    .refiner_token
                    .as_str(),
            );
            Some(true)
        }
        None => None,
    }
}

pub struct SearchEntry {
    pub bill_url: Url,
    pub bill_sponsor: String,
    pub bill_summary: String,
}

pub fn get_cell_data(table_row_element: &ElementRef, cell: i32) -> (String, Url) {
    // Most cell elements contains a hyperlink element ...
    let base_url = Url::parse("https://malegislature.gov").unwrap();
    let mut cell_selector = Selector::parse(format!("td:nth-child({cell}) a").as_str()).unwrap();
    match table_row_element.select(&cell_selector).next() {
        None => {
            // ... but if not, use the cell element, otherwise ...
            cell_selector = Selector::parse(format!("td:nth-child({cell})").as_str()).unwrap();
            let cell_element = table_row_element.select(&cell_selector).next().unwrap();
            (
                String::from(cell_element.text().collect::<Vec<_>>()[0].trim()),
                base_url,
            )
        }
        Some(cell_element) => (
            // ... use the hyperlink element
            String::from(cell_element.text().collect::<Vec<_>>()[0].trim()),
            base_url
                .join(cell_element.value().attr("href").unwrap())
                .unwrap(),
        ),
    }
}

pub fn get_and_print_search_results(url: &Url) -> IndexMap<String, SearchEntry> {
    // Use an IndexMap to preserve order
    let mut search_results_map = IndexMap::new();

    // Get the search result page, select the table, and parse each result row
    let body = reqwest::blocking::get(url.clone()).unwrap().text().unwrap();
    let document = Html::parse_document(body.as_str());
    let table_body_selector = Selector::parse("tbody").unwrap();
    let table_row_selector = Selector::parse("tr").unwrap();
    let table_body_element = document.select(&table_body_selector).next().unwrap();
    println!("Bill — Link — Sponsor — Summary");
    // TODO: Handle paging
    for table_row_element in table_body_element.select(&table_row_selector) {
        let (bill_number, bill_url) = get_cell_data(&table_row_element, 2);
        let (bill_sponsor, _) = get_cell_data(&table_row_element, 3);
        let (bill_summary, _) = get_cell_data(&table_row_element, 4);
        println!("{bill_number} — {bill_url} — {bill_sponsor} — {bill_summary}");

        // Collect each search result bill number, url, sponsor, and summary
        search_results_map.insert(
            bill_number,
            SearchEntry {
                bill_url,
                bill_sponsor,
                bill_summary,
            },
        );
    }
    search_results_map
}

pub fn get_and_print_bill_text_nodes(bill_url: &Url) -> Vec<String> {
    // Get the bill summary page
    let bill_body = reqwest::blocking::get(bill_url.clone())
        .unwrap()
        .text()
        .unwrap();
    let bill_document = Html::parse_document(bill_body.as_str());

    // Select the bill text URL
    let text_url_selector = Selector::parse("div.modalBtnGroup a:nth-child(1)").unwrap();
    let text_url_element = bill_document.select(&text_url_selector).next().unwrap();
    let text_url = Url::parse("https://malegislature.gov")
        .unwrap()
        .join(text_url_element.value().attr("href").unwrap());

    // Get the bill text page
    let text_body = reqwest::blocking::get(text_url.unwrap().clone())
        .unwrap()
        .text()
        .unwrap();
    let text_document = Html::parse_document(text_body.as_str());

    // Select, and print each paragraph of the bill text
    let text_selector = Selector::parse("div.modal-body div").unwrap();
    let text_element = text_document.select(&text_selector).next().unwrap();
    let mut text_nodes: Vec<String> = Vec::new();
    for text_node in text_element.text().collect::<Vec<_>>() {
        // TODO: Restore and make optional
        // println!("{text_node}");
        text_nodes.push(text_node.to_string());
    }
    text_nodes
}
pub fn write_bill_text_nodes(text_nodes: &Vec<String>, output_filename: String) {
    // Print each paragraph of the bill text to a file
    let path = Path::new(output_filename.as_str());
    let display = path.display();
    let mut file = match File::create(&path) {
        Err(why) => panic!("Couldn't create {}: {}", display, why),
        Ok(file) => file,
    };
    for text_node in text_nodes {
        match file.write(format!("{text_node}\n").as_bytes()) {
            Err(why) => panic!("Couldn't write to {}: {}", display, why),
            Ok(_) => (),
        }
    }
}

pub struct SectionRegex {
    section: Regex,
    amended: Regex,
    striking: Regex,
    inserting: Regex,
    repealed: Regex,
}
pub fn init_section_regex() -> SectionRegex {
    SectionRegex {
        section: Regex::new(r"SECTION").unwrap(),
        amended: Regex::new(r"amended").unwrap(),
        striking: Regex::new(r"striking").unwrap(),
        inserting: Regex::new(r"inserting").unwrap(),
        repealed: Regex::new(r"repealed").unwrap(),
    }
}

pub struct SectionCounts {
    pub total: i32,
    pub amending: i32,
    pub amending_by_striking: i32,
    pub amending_by_inserting: i32,
    pub amending_by_striking_and_inserting: i32,
    pub repealing: i32,
    pub other: i32,
}

pub fn init_section_counts() -> SectionCounts {
    SectionCounts {
        total: 0,
        amending: 0,
        amending_by_striking: 0,
        amending_by_inserting: 0,
        amending_by_striking_and_inserting: 0,
        repealing: 0,
        other: 0,
    }
}
pub fn count_sections(
    text_node: String,
    section_counts: &mut SectionCounts,
    section_regex: &SectionRegex,
    section_text: &mut String,
) {
    let text_str = text_node.as_str();
    if section_regex.section.is_match(text_str) {
        // Text starts a section of the bill
        section_counts.total += 1;
        if !section_text.is_empty() {
            // Section text has been accumulated
            let section_str = section_text.as_str();
            if section_regex.amended.is_match(section_str) {
                // Section amends an existing law
                section_counts.amending += 1;
                let is_striking = section_regex.striking.is_match(section_str);
                let is_inserting = section_regex.inserting.is_match(section_str);
                if is_striking && is_inserting {
                    // Section strikes out and inserts
                    section_counts.amending_by_striking_and_inserting += 1;
                } else if is_striking {
                    // Section strikes out only
                    section_counts.amending_by_striking += 1;
                } else if is_inserting {
                    // Section inserts only
                    section_counts.amending_by_inserting += 1;
                } else {
                    println!("NOT striking or inserting: {section_text}");
                }
            } else if section_regex.repealed.is_match(section_text.as_str()) {
                // Section repeals an existing law
                section_counts.repealing += 1;
            } else {
                // Section establishes a new law
                section_counts.other += 1;
                println!("NOT amending or repealing: {section_text}");
            }
        }
        section_text.clear();
    }
    // Text continues a section of the bill
    section_text.push_str(text_str);
}
