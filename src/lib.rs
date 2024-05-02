use clap::Parser;
use indexmap::IndexMap;
use log::{debug, error, info};
use regex::Regex;
use scraper::{Element, ElementRef, Html, Selector};
use std::collections::hash_map::Entry;
use std::collections::HashMap;

use std::fs::File;
use std::hash::Hash;
use std::io::Write;
use std::path::Path;
use std::sync::mpsc;

use std::sync::mpsc::Sender;
use std::{fs, thread};
use url::quirks::search;
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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
    // TODO: Handle paging?
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

pub fn get_section_key(chapter: &String, section: &String) -> String {
    String::from(chapter.to_string() + "-" + section)
}

pub fn get_bill_text_nodes(bill_url: &Url) -> Vec<String> {
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
        .join(text_url_element.value().attr("href").unwrap().trim())
        .unwrap();
    info!("Value for text URL: {}", text_url);

    // Get the bill text page
    let text_body = reqwest::blocking::get(text_url).unwrap().text().unwrap();
    let text_document = Html::parse_document(text_body.as_str());

    // Select, and (optionally) print each text node of the bill text
    let container_selector = Selector::parse("div.modal-body div").unwrap();
    let container_element = text_document.select(&container_selector).next().unwrap();
    let mut text_nodes: Vec<String> = Vec::new();
    for text_node in container_element.text().collect::<Vec<_>>() {
        // TODO: Restore and make optional
        // println!("{text_node}");
        text_nodes.push(text_node.to_string());
    }
    text_nodes
}

pub fn write_text_nodes(text_nodes: &Vec<String>, output_filename: String, output_folder: &String) {
    // Print each text node of the bill to a file
    fs::create_dir_all(output_folder);
    let str_path = [output_folder, output_filename.as_str()].join("/");
    let path = Path::new(&str_path);
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

// TODO: Document these?
pub fn init_markup_regex() -> MarkupRegex {
    MarkupRegex {
        text_parse: Regex::new(r"((?i)section.*)[\n\s]*([\S\s]*)").unwrap(),
        striking: Regex::new(r"strik").unwrap(),
        inserting: Regex::new(r"insert").unwrap(),
        words: Regex::new(r"words?").unwrap(),
        sections: Regex::new(r"sections?:").unwrap(),
        subsections: Regex::new(r"(subsections?|subclauses?):").unwrap(),
        lines: Regex::new(r"lines?").unwrap(),
        repealed: Regex::new(r"repealed ?(.*)").unwrap(),
        replace_words: Regex::new(r#"strik.*(“|")(.*)(”|").*insert.*?:-? (.*)\."#).unwrap(),
        replace_lines: Regex::new(r#"strik.*lines (\d*)[^\d]*(\d*).*insert.*?:-?(.*)"#).unwrap(),
        replace_section: Regex::new(r"strik?.*section.*insert.*?:-?(.*)").unwrap(),
        replace_subsection: Regex::new(
            r"strik?.*(subsection|subclause) \((.)\).*insert.*?:-?([\s\S]*)",
        )
        .unwrap(),
        strike_words: Regex::new(r#"strik.*(“|")(.*)(”|")?\."#).unwrap(),
        strike_lines: Regex::new(r"strike_lines").unwrap(),
        strike_section: Regex::new(r"strike_section").unwrap(),
        insert_words: Regex::new(r#"insert.*word.*(“|")(.*)(”|").*.*?:-? (.*)\."#).unwrap(),
        insert_lines: Regex::new(r"insert_lines").unwrap(),
        insert_section: Regex::new(r"insert_section").unwrap(),
    }
}

fn mark_text(
    law_section_text: &String,
    bill_section_text: &String,
    bill_section_number: &String,
    markup_regex: &MarkupRegex,
) -> String {
    // Section amends an existing law
    let is_repealing = markup_regex.repealed.is_match(&*bill_section_text);
    let is_striking = markup_regex.striking.is_match(&*bill_section_text);
    let is_inserting = markup_regex.inserting.is_match(&*bill_section_text);
    let is_words = markup_regex.words.is_match(&*bill_section_text);
    let is_sections = markup_regex.sections.is_match(&*bill_section_text);
    let is_subsections = markup_regex.subsections.is_match(&*bill_section_text);
    let is_lines = markup_regex.lines.is_match(&*bill_section_text);
    let mut marked_text = law_section_text.clone();

    // Repealing
    if is_repealing {
        if let Some(caps) = markup_regex.repealed.captures(bill_section_text.as_ref()) {
            let repeal_specifications = String::from(&caps[1]);

            marked_text = format!(
                "\
            [.line-through .red]#{law_section_text}#^{bill_section_number}^\n\nREPEALED {repeal_specifications}
            "
            )
        }
    }
    // Striking and Inserting
    else if is_striking && is_inserting {
        // Striking and inserting words
        if is_words {
            if let Some(caps) = markup_regex
                .replace_words
                .captures(bill_section_text.as_ref())
            {
                let striked_words = String::from(&caps[2]);
                let inserted_words = String::from(&caps[4]);
                let mut buffer = "";

                // Handle asciidoc not marking up document if buffer before class not present
                if striked_words.starts_with([',', '.', ':', ' ']) {
                    buffer = " ";
                }
                // Format replacement
                let replacement = format!(
                    "\
                    {buffer}[.line-through .red]#{striked_words}# \
                    [.blue]#{inserted_words}#^{bill_section_number}^\
                    "
                );

                marked_text = law_section_text.replace(&striked_words, &*replacement)
            }
        }
        // Striking and inserting line(s)
        else if is_lines {
            if let Some(caps) = markup_regex
                .replace_lines
                .captures(bill_section_text.as_ref())
            {
                let strike_start_line = String::from(&caps[1]);
                let strike_end_line = String::from(&caps[2]);
                let inserted_words = String::from(&caps[3]);

                //TODO: figure out how to convert line numbers into actual strings
                let striked_words = String::from("PLACEHOLDER");

                // Format replacement
                let replacement = format!(
                    "\
                [.line-through .red]#{striked_words}# \
                [.blue]#{inserted_words}#^{bill_section_number}^\
                "
                );

                marked_text = law_section_text.replace(&striked_words, &*replacement)
            }
        }
        // Striking and inserting subsections(s)
        else if is_subsections {
            if let Some(caps) = markup_regex
                .replace_subsection
                .captures(bill_section_text.as_ref())
            {
                let subsection_char = String::from(caps[2].trim());
                let insert = String::from(caps[3].trim());

                // Create string for regex
                let get_subsection_regex_string = format!(
                    r"(?i)(\n|^)(section \d+.\s*)?(\({}\))([\s\S]*?)\n(\[.*\]|\([^\d\W]\))",
                    subsection_char
                );
                let get_subsection_regex =
                    Regex::new(get_subsection_regex_string.as_ref()).unwrap();
                if let Some(caps) = get_subsection_regex.captures(law_section_text.as_ref()) {
                    let subsection_header = String::from(caps[3].trim());
                    let subsection_content = String::from(caps[4].trim());
                    let subsection = format!("{} {}", subsection_header, subsection_content);

                    // Format replacement
                    let mut replacement = format!(
                        "\
                [.line-through .red]##{subsection}##\n\n[.blue]##{insert}##^{bill_section_number}^\
                "
                    );
                    replacement = replacement.replace("\n", " +\n");

                    marked_text = law_section_text.replace(&subsection, &*replacement)
                }
            }
        }
        // Striking and inserting section(s)
        else if is_sections {
            if let Some(caps) = markup_regex
                .replace_section
                .captures(bill_section_text.as_ref())
            {
                let insert = String::from(caps[1].trim());
                // Format replacement
                marked_text = format!(
                    "\
                [.line-through .red]#{law_section_text}#\n\n[.blue]#{insert}#^{bill_section_number}^\
                "
                )
            }
        }
    }
    // Striking
    else if is_striking {
        // Striking words
        if is_words {
            if let Some(caps) = markup_regex
                .strike_words
                .captures(bill_section_text.as_ref())
            {
                let striked_words = String::from(&caps[2]);
                // Format replacement
                let replacement = format!(
                    "\
                    [.line-through .red]#{striked_words}#^{bill_section_number}^ \
                    "
                );

                marked_text = law_section_text.replace(&striked_words, &*replacement)
            }
        }
        // Striking line(s)
        else if is_lines {
            println!("Striking lines not implemented!")
        }
        // Striking section(s)
        else if is_sections {
            println!("Striking sections not implemented!")
        }
    }
    // Inserting
    else if is_inserting {
        // Inserting words
        if is_words {
            println!("Inserting words not implemented!")
        }
        // Inserting line(s)
        else if is_lines {
            println!("Inserting lines not implemented!")
        }
        // Inserting section(s)
        else if is_sections {
            println!("Inserting sections not implemented!")
        }
    } else {
        println!("Not sure what section does: {}", &*law_section_text);
    }
    marked_text
}
fn mark_section_text(
    law_section: &LawSectionWithText,
    bill_sections: &Vec<BillSection>,
    markup_regex: &MarkupRegex,
) -> Option<String> {
    // Parse law section title and contents
    if let Some(caps) = markup_regex.text_parse.captures(law_section.text.as_ref()) {
        let title = String::from(caps[1].trim());
        let law_section_text = String::from(caps[2].trim());

        let mut marked_text = law_section_text.clone();

        // Apply markups for law_section across all applicable bill sections
        for bill_section_key in &law_section.bill_section_keys {
            if let Some(bill_section) = bill_sections
                .iter()
                .find(|bill_section| &bill_section.section_number == bill_section_key)
            {
                // need to sort out law section vs bill section
                marked_text = mark_text(
                    &marked_text,
                    &bill_section.text,
                    &bill_section.section_number,
                    markup_regex,
                );
            }
        }

        let marked_section_text = format!("*{title}*\n\n{marked_text}");
        return Some(marked_section_text);
    }
    None
}

pub fn write_asciidocs(
    law_sections_text: Vec<LawSectionWithText>,
    bill_sections_text: &Vec<BillSection>,
    markup_regex: &MarkupRegex,
    output_folder: String,
) -> std::io::Result<()> {
    for law_section in law_sections_text {
        let file_name = &law_section.law_chapter_key;
        if let Some(marked_text) = mark_section_text(&law_section, bill_sections_text, markup_regex)
        {
            fs::create_dir_all(format!("{output_folder}/modified-laws"));
            let mut file = File::create(format!("{output_folder}/modified-laws/{file_name}.adoc"))?;
            file.write_all(marked_text.as_ref())?;
        } else {
            println!("Could not mark up law section: {file_name}")
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
pub struct SectionRegex {
    bill_section: Regex,
    amended: Regex,
    striking: Regex,
    inserting: Regex,
    repealed: Regex,
    law_chapter: Regex,
    law_section: Regex,
    section_list: Regex,
}

// TODO: Document these?
pub fn init_section_regex() -> SectionRegex {
    SectionRegex {
        bill_section: Regex::new(r"^\s*SECTION\s*(\d*\w*)\s*\.").unwrap(),
        amended: Regex::new(r"amended").unwrap(),
        striking: Regex::new(r"striking").unwrap(),
        inserting: Regex::new(r"inserting").unwrap(),
        repealed: Regex::new(r"repealed").unwrap(),
        law_chapter: Regex::new(
            r"^\s*(?i)section [\d+][^a-z](?-i)\.*[^\.:-]*?[cC]hapter\s*(\d*\w*)",
        )
        .unwrap(),
        law_section: Regex::new(
            r"^\s*(?i)section [\d+][^a-z](?-i)\.*[^\.:-]*?([sS]ection[s]*)\s*(\d*\w*)",
        )
        .unwrap(),
        section_list: Regex::new(r"(\d+\w*\s*[\u00BC-\u00BE\u2150-\u215E]*)[,\s]").unwrap(),
    }
}

#[derive(Debug, Clone)]
pub struct MarkupRegex {
    text_parse: Regex,
    striking: Regex,
    inserting: Regex,
    words: Regex,
    sections: Regex,
    subsections: Regex,
    lines: Regex,
    repealed: Regex,
    replace_words: Regex,
    replace_lines: Regex,
    replace_section: Regex,
    replace_subsection: Regex,
    strike_words: Regex,
    strike_lines: Regex,
    strike_section: Regex,
    insert_words: Regex,
    insert_lines: Regex,
    insert_section: Regex,
}

#[derive(Debug, Clone, Copy)]
pub struct SectionCounts {
    pub total: i32,
    pub amending: i32,
    pub amending_by_striking: i32,
    pub amending_by_inserting: i32,
    pub amending_by_striking_and_inserting: i32,
    pub repealing: i32,
    pub other: i32,
}

fn init_section_counts() -> SectionCounts {
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

pub fn collect_bill_sections(
    text_nodes: Vec<String>,
    section_regex: &SectionRegex,
) -> Vec<BillSection> {
    let mut bill = Vec::new();
    let mut section_text = String::new();

    for text_node in text_nodes {
        let text_str = text_node.as_str();
        if section_regex.bill_section.is_match(text_str) {
            // Indicates section_text is a complete section of bill
            if !section_text.is_empty() {
                // Collect bill section
                collect_bill_section(&section_text, section_regex, &mut bill);
            }
            section_text.clear();
        }
        if section_text.is_empty() {
            section_text.push_str(text_str);
        } else {
            section_text.push_str(&*format!("\n{}", text_str))
        }
    }
    // Collect final bill section
    collect_bill_section(&section_text, section_regex, &mut bill);
    bill
}

fn collect_bill_section(
    section_text: &String,
    section_regex: &SectionRegex,
    bill: &mut Vec<BillSection>,
) {
    let section_str = section_text.as_str();
    let mut section_number = String::from("");
    if let Some(caps) = section_regex.bill_section.captures(section_str) {
        section_number = String::from(&caps[1]);
    } else {
        println!("{section_str}");
    }
    let law_sections = collect_law_sections(&section_number, section_regex, section_str);
    let bill_section = BillSection {
        section_number,
        text: section_text.to_string(),
        law_sections,
    };
    bill.push(bill_section)
}

pub fn count_bill_section_types(
    bill: &Vec<BillSection>,
    section_regex: &SectionRegex,
) -> SectionCounts {
    let mut section_counts = init_section_counts();
    section_counts.total = bill.len() as i32;
    for bill_section in bill {
        // println!("Bill Section: {:?}", bill_section);
        if section_regex.amended.is_match(&*bill_section.text) {
            // Section amends an existing law
            section_counts.amending += 1;
            let is_striking = section_regex.striking.is_match(&*bill_section.text);
            let is_inserting = section_regex.inserting.is_match(&*bill_section.text);
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
                println!("NOT striking or inserting: {}", &*bill_section.text);
            }
        } else {
            let is_repealing = section_regex.repealed.is_match(&*bill_section.text);
            // Section repeals an existing law
            if is_repealing {
                section_counts.repealing += 1;
            } else {
                section_counts.other += 1;
            }
        }
    }
    println!("Total sections: {}", section_counts.total);
    println!("Amending sections: {}", section_counts.amending);
    println!(
        "Amending sections by striking and inserting: {}",
        section_counts.amending_by_striking_and_inserting
    );
    println!(
        "Amending sections by striking: {}",
        section_counts.amending_by_striking
    );
    println!(
        "Amending sections by inserting: {}",
        section_counts.amending_by_inserting
    );
    println!("Repealing sections: {}", section_counts.repealing);
    println!("Other sections: {}", section_counts.other);
    section_counts
}

#[derive(Debug)]
pub struct BillSection {
    pub section_number: String,
    pub text: String,
    pub law_sections: LawSections,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LawSections {
    pub chapter_number: String,
    pub section_numbers: Vec<String>,
}

fn collect_law_sections(
    _bill_section_number: &str,
    section_regex: &SectionRegex,
    section_str: &str,
) -> LawSections {
    // Capture law chapter
    let mut law_chapter = String::from("");
    if let Some(caps) = section_regex.law_chapter.captures(section_str) {
        law_chapter = String::from(&caps[1]);
    } else {
        //TODO: Handle this as error instead
        println!("{section_str}");
    }
    // Exit if no chapter found
    if law_chapter == "" {
        return LawSections {
            chapter_number: law_chapter,
            section_numbers: Vec::new(),
        };
    }
    // Capture law sections
    let mut law_sections: Vec<String> = Vec::new();
    if let Some(caps) = section_regex.law_section.captures(section_str) {
        if caps[1].trim().to_lowercase().eq("section") {
            // Found a single section
            law_sections.push(String::from((&caps[2]).trim_end()));
        } else if caps[1].trim().to_lowercase().eq("sections") {
            // Found multiple, comma delimited sections
            let mut sections: Vec<_> = section_regex
                .section_list
                .find_iter(section_str)
                .map(|m| m.as_str())
                .map(|s| s.trim_end_matches(",").trim_end())
                .map(|s| String::from(s))
                .collect();
            law_sections.append(&mut sections);
        } else {
            //TODO: Handle this as error instead
            println!("{section_str}");
        }
    } else {
        //TODO: Handle this as error instead
        println!("{section_str}");
    }
    println!("{:?}, {}", law_sections, law_chapter);
    LawSections {
        chapter_number: law_chapter,
        section_numbers: law_sections,
    }
}
pub fn get_search_page(cli: &Cli) -> (bool, Url, String) {
    // Get default search page, parse, and create refiner map
    info!("Creating refiner map");
    let refiner_map = create_refiner_map();

    // Construct search URL
    let mut search_url = Url::parse("https://malegislature.gov/Bills/Search").unwrap();

    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1
    // https://malegislature.gov/Bills/Search?SearchTerms=mbta&Page=1
    let mut do_search;
    let search_term = match cli.search_term.as_deref() {
        None => {
            do_search = false;
            String::from("")
        }
        Some(search_term) => {
            do_search = true;
            search_term.to_string()
        }
    };
    search_url
        .query_pairs_mut()
        .append_pair("SearchTerms", search_term.as_str())
        .append_pair("Page", "1");

    // https://malegislature.gov/Bills/Search
    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1&Refinements%5Blawsgeneralcourt%5D=3139326e64202832303231202d203230323229
    print_entries_or_append_query_pair(
        cli.general_court.as_deref(),
        refiner_map.get("General Court").unwrap(),
        &mut String::from("Refinements[lawsgeneralcourt]"),
        &mut search_url,
    );

    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1&Refinements%5Blawsbranchname%5D=486f757365
    do_search = match print_entries_or_append_query_pair(
        cli.branch.as_deref(),
        refiner_map.get("Branch").unwrap(),
        &mut String::from("Refinements[lawsbranchname]"),
        &mut search_url,
    ) {
        None => do_search,
        Some(do_search) => do_search,
    };

    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1&Refinements%5Blawsuserprimarysponsorname%5D=4172636965726f2c204a616d6573
    do_search = match print_entries_or_append_query_pair(
        cli.sponsor_legislator.as_deref(),
        refiner_map.get("Sponsor — Legislator").unwrap(),
        &mut String::from("Refinements[lawsuserprimarysponsorname]"),
        &mut search_url,
    ) {
        None => do_search,
        Some(do_search) => do_search,
    };

    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1&Refinements%5Blawscommitteeprimarysponsorname%5D=3139326e64204a52756c6573
    do_search = match print_entries_or_append_query_pair(
        cli.sponsor_committee.as_deref(),
        refiner_map.get("Sponsor — Committee").unwrap(),
        &mut String::from("Refinements[lawscommitteeprimarysponsorname]"),
        &mut search_url,
    ) {
        None => do_search,
        Some(do_search) => do_search,
    };

    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1&Refinements%5Blawsotherprimarysponsorname%5D=41756469746f72206f662074686520436f6d6d6f6e7765616c7468
    do_search = match print_entries_or_append_query_pair(
        cli.sponsor_other.as_deref(),
        refiner_map.get("Sponsor — Other").unwrap(),
        &mut String::from("Refinements[lawsotherprimarysponsorname]"),
        &mut search_url,
    ) {
        None => do_search,
        Some(do_search) => do_search,
    };

    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1&Refinements%5Blawsfilingtype%5D=416d656e646d6%%6e74
    do_search = match print_entries_or_append_query_pair(
        cli.document_type.as_deref(),
        refiner_map.get("Document Type").unwrap(),
        &mut String::from("Refinements[lawsfilingtype]"),
        &mut search_url,
    ) {
        None => do_search,
        Some(do_search) => do_search,
    };
    (do_search, search_url, search_term)
}

pub struct LawSectionWithText {
    law_chapter_key: String,
    text: String,
    bill_section_keys: Vec<String>,
}

pub fn get_required_law_sections(bill: &Vec<BillSection>) -> Vec<LawSectionWithText> {
    // Iterate through bill to get list of all needed sections for downloading
    let mut required_law_sections: Vec<(String, String)> = Vec::new();
    let mut law_section_bill_sections: HashMap<String, Vec<String>> = HashMap::new();
    for bill_section in bill {
        for section in &bill_section.law_sections.section_numbers {
            let chapter = bill_section.law_sections.chapter_number.clone();
            let section_key = get_section_key(&chapter, section);

            match law_section_bill_sections.entry(section_key) {
                Entry::Vacant(e) => {
                    e.insert(vec![String::from(&bill_section.section_number)]);
                }
                Entry::Occupied(mut e) => {
                    e.get_mut().push(String::from(&bill_section.section_number));
                }
            }

            // Push required law section to list
            // TODO: Review
            required_law_sections.push((chapter, section.to_string()))
        }
    }
    // Remove duplicates
    required_law_sections.sort();
    required_law_sections.dedup();

    // Download required law sections concurrently
    let (tx, rx) = mpsc::channel();
    let (chapter, section) = required_law_sections.pop().unwrap();
    for (chapter, section) in required_law_sections {
        download_law_section(&chapter, &section, tx.clone());
    }
    // Download final law section
    download_law_section(&chapter, &section, tx);

    // Collect law sections and create struct
    let mut law_sections_text: Vec<LawSectionWithText> = vec![];
    for (chapter, section, text) in rx {
        println!("Got law section: {:?} of chapter {:?}", section, chapter);
        let law_chapter_key = get_section_key(&chapter, &section);
        let bill_sections = law_section_bill_sections.get(&law_chapter_key);
        match bill_sections {
            Some(b) => {
                let law_section_text = LawSectionWithText {
                    law_chapter_key,
                    text,
                    bill_section_keys: b.to_vec(),
                };
                law_sections_text.push(law_section_text);
            }
            None => {
                // TODO: This should not happen?
                let law_section_text = LawSectionWithText {
                    law_chapter_key,
                    text,
                    bill_section_keys: Vec::new(),
                };
                law_sections_text.push(law_section_text);
            }
        }
    }

    law_sections_text
}

pub fn format_law_section(law_section: &String) -> String {
    // Format law sections containing unicode vulgar fractions for use in going to law section
    let last_char = law_section.chars().last().unwrap();
    let all_but_last_char = law_section
        .trim_end_matches(last_char)
        .trim_end()
        .to_owned();
    match last_char {
        '¼' => all_but_last_char + " 1~4",
        '½' => all_but_last_char + " 1~2",
        '¾' => all_but_last_char + " 3~4",
        '⅐' => all_but_last_char + " 1~7",
        '⅑' => all_but_last_char + " 1~9",
        '⅒' => all_but_last_char + " 1~10",
        '⅓' => all_but_last_char + " 1~3",
        '⅔' => all_but_last_char + " 2~3",
        '⅕' => all_but_last_char + " 1~5",
        '⅖' => all_but_last_char + " 2~5",
        '⅗' => all_but_last_char + " 3~5",
        '⅘' => all_but_last_char + " 4~5",
        '⅙' => all_but_last_char + " 1~6",
        '⅚' => all_but_last_char + " 5~6",
        '⅛' => all_but_last_char + " 1~8",
        '⅜' => all_but_last_char + " 3~8",
        '⅝' => all_but_last_char + " 5~8",
        '⅞' => all_but_last_char + " 7~8",
        _ => law_section.to_string(),
    }
}

pub fn download_law_section(
    law_chapter: &String,
    law_section: &String,
    tx: Sender<(String, String, String)>,
) {
    // Clone input arguments and move into the spawned thread closure
    let law_chapter = law_chapter.clone();
    let law_section = law_section.clone();
    thread::spawn(move || {
        // Construct the law URL
        let mut law_url = Url::parse("https://malegislature.gov/GeneralLaws/GoTo").unwrap();
        law_url
            .query_pairs_mut()
            .append_pair("ChapterGoTo", law_chapter.as_str())
            .append_pair("SectionGoTo", format_law_section(&law_section).as_str());
        info!("Value for law URL: {}", law_url);

        // Get and parse the law page
        let body = reqwest::blocking::get(law_url.clone())
            .unwrap()
            .text()
            .unwrap();
        let document = Html::parse_document(body.as_str());

        // Find the text node container
        let h2_selector = Selector::parse("h2#skipTo").unwrap();
        let h2_element = match document.select(&h2_selector).next() {
            Some(element) => element,
            None => panic!("Cannot get element for URL {}", law_url),
        };
        let container_element = h2_element.parent_element().unwrap();

        // Collect the law text nodes
        let mut law_text = String::new();
        for text_node in container_element.text().collect::<Vec<_>>() {
            law_text.push_str(text_node);
        }

        tx.send((law_chapter, law_section, law_text)).unwrap();
    });
}
