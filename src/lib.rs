mod bill_section;
mod law_section;
mod ma_legislature;
mod markup;

use crate::{bill_section::BillSection, markup::MarkupRegex};
use clap::Parser;
use fancy_regex::Regex;
use indexmap::IndexMap;
use log::{debug, error, info};
use scraper::{Element, ElementRef, Html, Selector};
use std::{
    collections::{hash_map::Entry, HashMap},
    error::Error,
    fs,
    fs::File,
    hash::Hash,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
    sync::{mpsc, mpsc::Sender},
    thread,
};
use url::{quirks::search, Url};

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

pub fn create_search_results_map(
    cli: &Cli,
) -> (IndexMap<String, ma_legislature::SearchEntry>, String) {
    // Parse command line arguments and construct search URL
    info!("Constructing search URL");
    let (do_search, search_url, search_term) = ma_legislature::get_search_page(&cli);

    // Get and print the search results
    let mut search_results_map = IndexMap::new();
    if do_search || cli.list {
        info!("Value for search URL: {search_url}");
        search_results_map = ma_legislature::get_and_print_search_results(&search_url);
    }
    // Return search results and term
    (search_results_map, search_term)
}

pub fn create_bill(search_entry: &ma_legislature::SearchEntry) -> Vec<BillSection> {
    let bill_url = &search_entry.bill_url;
    info!("Value for bill URL: {bill_url}");
    let text_nodes = bill_section::get_bill_text_nodes(bill_url);

    // Collect bill sections and law sections into structs with regex
    let section_regex = bill_section::init_bill_section_regex();
    let bill = bill_section::collect_bill_sections(&text_nodes, &section_regex);

    // Count and print type of bill sections with regex
    let section_counts = bill_section::count_bill_section_types(&bill, &section_regex);
    bill_section::print_bill_section_types(section_counts);
    bill
}

pub fn create_law_sections_text(bill: &Vec<BillSection>) -> Vec<law_section::LawSectionWithText> {
    // Iterate through bill to get list of all needed sections for downloading
    let mut required_law_sections: Vec<(String, String)> = Vec::new();
    let mut law_section_bill_sections: HashMap<String, Vec<String>> = HashMap::new();
    for bill_section in bill {
        for law_section in &bill_section.law_sections.section_numbers {
            let law_chapter = bill_section.law_sections.chapter_number.clone();
            let section_key = law_section::get_section_key(&law_chapter, law_section);

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
            required_law_sections.push((law_chapter, law_section.to_string()))
        }
    }
    // Remove duplicates
    required_law_sections.sort();
    required_law_sections.dedup();

    // Download required law sections concurrently
    let (tx, rx) = mpsc::channel();
    let (law_chapter, law_section) = required_law_sections.pop().unwrap();
    for (law_chapter, law_section) in required_law_sections {
        law_section::download_law_section(&law_chapter, &law_section, tx.clone());
    }
    // Download final law section
    law_section::download_law_section(&law_chapter, &law_section, tx);

    // Collect law sections and create struct
    let mut law_sections_text: Vec<law_section::LawSectionWithText> = vec![];
    for (law_chapter, law_section, text) in rx {
        println!(
            "Got law section: {:?} of chapter {:?}",
            law_section, law_chapter
        );
        let law_chapter_key = law_section::get_section_key(&law_chapter, &law_section);
        let bill_sections = law_section_bill_sections.get(&law_chapter_key);
        match bill_sections {
            Some(b) => {
                let law_section_text = law_section::LawSectionWithText {
                    law_chapter_key,
                    text,
                    bill_section_keys: b.to_vec(),
                };
                law_sections_text.push(law_section_text);
            }
            None => {
                // TODO: This should not happen?
                let law_section_text = law_section::LawSectionWithText {
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
pub fn write_bill(bill: &Vec<BillSection>, output_filename: String, output_folder: &String) {
    // Print each text node of the bill to a file
    fs::create_dir_all(output_folder);
    let str_path = [output_folder, output_filename.as_str()].join("/");
    let path = Path::new(&str_path);
    let display = path.display();
    let mut file = match File::create(&path) {
        Err(why) => panic!("Couldn't create {}: {}", display, why),
        Ok(file) => file,
    };
    for bill_section in bill {
        match file.write(format!("{}\n", bill_section.text).as_bytes()) {
            Err(why) => panic!("Couldn't write to {}: {}", display, why),
            Ok(_) => (),
        }
    }
}
pub fn write_asciidocs(
    law_sections_text: Vec<law_section::LawSectionWithText>,
    bill_sections_text: &Vec<BillSection>,
    output_folder: &String,
    law_folder: &str,
) -> std::io::Result<()> {
    let markup_regex = markup::init_markup_regex();
    for law_section in law_sections_text {
        let file_name = &law_section.law_chapter_key;
        if let Some(marked_text) =
            markup::mark_section_text(&law_section, bill_sections_text, &markup_regex)
        {
            fs::create_dir_all(format!("{output_folder}/{law_folder}"));
            let mut file = File::create(format!("{output_folder}/modified-laws/{file_name}.adoc"))?;
            file.write_all(marked_text.as_ref())?;
        } else {
            println!("Could not mark up law section: {file_name}")
        }
    }
    Ok(())
}

pub fn run_asciidoctor(output_folder: String, law_folder: &str) -> () {
    let paths = markup::get_adoc_paths(&format!("{output_folder}/{law_folder}")).unwrap();

    for path in paths {
        let mut asciidoctor = Command::new("sh");
        asciidoctor.arg("asciidoctor").arg(path.as_os_str());
        let _ = asciidoctor.output().expect(
            "Failed to parse file - is asciidoctor installed? (i.e. ~brew install asciidoctor)",
        );
    }
}
