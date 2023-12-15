use clap::Parser;
use indexmap::IndexMap;
use log::info;
use std::collections::HashMap;
use std::sync::mpsc;
use url::Url;

use springbok_mgl::*;

fn main() {
    env_logger::init();

    // Get default search page, parse, and create refiner map
    info!("Creating refiner map");
    let refiner_map = create_refiner_map();

    // Parse command line arguments and construct search URL
    info!("Constructing search URL");
    let cli = Cli::parse();
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

    // Get and print the search results
    let mut search_results_map = IndexMap::new();
    if do_search || cli.list {
        info!("Value for search URL: {search_url}");
        search_results_map = get_and_print_search_results(&search_url);
    }

    // Get, and print the bill text when searching by bill number
    if cli.download {
        if let Some(&ref search_entry) = search_results_map.get(search_term.as_str()).as_deref() {
            let bill_url = &search_entry.bill_url;
            info!("Value for bill URL: {bill_url}");
            let text_nodes = get_bill_text_nodes(bill_url);

            // Write the bill text when to a file
            if let Some(output_filename) = cli.output_filename {
                write_text_nodes(&text_nodes, output_filename);
            }

            // Count bill sections that amend and repeal existing law and collect corresponding law and bill locations
            let section_regex = init_section_regex();

            let unpopulated_bill = collect_bill_sections(text_nodes, &section_regex);

            let section_counts = count_bill_section_types(&unpopulated_bill, &section_regex);

            // Iterate through unpopulated bill to get flat list of all needed sections
            let mut flat_law_chapter_sections: Vec<(String, String)> = Vec::new();
            for bill_section in &unpopulated_bill {
                for section in &bill_section.law_sections.sections {
                    flat_law_chapter_sections.push((
                        bill_section.law_sections.chapter_number.clone(),
                        section.to_string(),
                    ))
                }
            }
            // Remove duplicates
            flat_law_chapter_sections.sort();
            flat_law_chapter_sections.dedup();

            // Get law sections concurrently
            let (tx, rx) = mpsc::channel();
            let chapter_section = flat_law_chapter_sections.pop().unwrap();
            for chapter_section in flat_law_chapter_sections {
                get_law_section(&chapter_section.0, &chapter_section.1, tx.clone());
            }
            // Get final law section
            get_law_section(&chapter_section.0, &chapter_section.1, tx);

            let mut law_chapter_sections_text: HashMap<String, String> = HashMap::new();
            for law_section_string in rx {
                println!(
                    "Got law section: {:?} of chapter {:?}",
                    law_section_string.0, law_section_string.1
                );
                law_chapter_sections_text.insert(
                    String::from(law_section_string.0 + "-" + &*law_section_string.1),
                    law_section_string.2,
                );
            }

            let mut populated_bill: Vec<BillSection> = Vec::new();
            // Populate BillSection structs with LawSections HashMap
            for bill_section in unpopulated_bill {
                let mut text = HashMap::new();
                for section in &bill_section.law_sections.sections {
                    let query = bill_section.law_sections.chapter_number.clone()
                        + &"-".to_string()
                        + section;
                    text.insert(
                        section.clone(),
                        match law_chapter_sections_text.get(&query) {
                            Some(result) => result,
                            None => "",
                        }
                        .to_string(),
                    );
                }
                let populated_law_section = LawSections {
                    bill_section_number: bill_section.law_sections.bill_section_number,
                    chapter_number: bill_section.law_sections.chapter_number,
                    sections: bill_section.law_sections.sections,
                    text,
                };
                let populated_bill_section = BillSection {
                    section_number: bill_section.section_number,
                    text: bill_section.text,
                    law_sections: populated_law_section,
                };
                populated_bill.push(populated_bill_section);
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
        } else {
            info!("Search term is not a bill number")
        }
    }
}
