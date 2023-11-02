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
        None => None
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

pub fn download_bill_text(bill_url: &Url) {
    // Get the bill summary page
    let bill_body = reqwest::blocking::get(bill_url.clone())
        .unwrap()
        .text()
        .unwrap();
    let bill_document = Html::parse_document(bill_body.as_str());

    // Select the bill text page
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
    for node in text_element.text().collect::<Vec<_>>() {
        println!("{node}");
    }
}
