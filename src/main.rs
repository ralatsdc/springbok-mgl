use clap::Parser;
use log::info;
use url::Url;

use springbok_mgl::{create_refiner_map, list_entries_or_append_query_pair, Cli, get_search_results};

fn main() {
    env_logger::init();

    // Get default search page, parse, and create refiner map

    info!("Creating refiner map");
    let refiner_map = create_refiner_map();

    // Parsing command line arguments and construct search URL

    info!("Constructing search URL");
    let cli = Cli::parse();
    let mut search_url = Url::parse("https://malegislature.gov/Bills/Search").unwrap();

    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1
    // https://malegislature.gov/Bills/Search?SearchTerms=mbta&Page=1
    match cli.search_term {
        None => search_url.set_query(Some("SearchTerms=&Page=1")),
        Some(search_term) => {
            search_url.set_query(Some(format!("SearchTerms={search_term}&Page=1").as_str()))
        }
    }

    // https://malegislature.gov/Bills/Search
    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1&Refinements%5Blawsgeneralcourt%5D=3139326e64202832303231202d203230323229
    list_entries_or_append_query_pair(
        &cli.general_court,
        refiner_map.get("General Court").unwrap(),
        &mut String::from("Refinements[lawsgeneralcourt]"),
        &mut search_url,
    );

    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1&Refinements%5Blawsbranchname%5D=486f757365
    list_entries_or_append_query_pair(
        &cli.branch,
        refiner_map.get("Branch").unwrap(),
        &mut String::from("Refinements[lawsbranchname]"),
        &mut search_url,
    );

    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1&Refinements%5Blawsuserprimarysponsorname%5D=4172636965726f2c204a616d6573
    list_entries_or_append_query_pair(
        &cli.sponsor_legislator,
        refiner_map.get("Sponsor — Legislator").unwrap(),
        &mut String::from("Refinements[lawsuserprimarysponsorname]"),
        &mut search_url,
    );

    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1&Refinements%5Blawscommitteeprimarysponsorname%5D=3139326e64204a52756c6573
    list_entries_or_append_query_pair(
        &cli.sponsor_committee,
        refiner_map.get("Sponsor — Committee").unwrap(),
        &mut String::from("Refinements[lawscommitteeprimarysponsorname]"),
        &mut search_url,
    );

    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1&Refinements%5Blawsotherprimarysponsorname%5D=41756469746f72206f662074686520436f6d6d6f6e7765616c7468
    list_entries_or_append_query_pair(
        &cli.sponsor_other,
        refiner_map.get("Sponsor — Other").unwrap(),
        &mut String::from("Refinements[lawsotherprimarysponsorname]"),
        &mut search_url,
    );

    // https://malegislature.gov/Bills/Search?SearchTerms=&Page=1&Refinements%5Blawsfilingtype%5D=416d656e646d6%%6e74
    list_entries_or_append_query_pair(
        &cli.document_type,
        refiner_map.get("Document Type").unwrap(),
        &mut String::from("Refinements[lawsfilingtype]"),
        &mut search_url,
    );
    info!("Value for search URL: {search_url}");

    get_search_results(search_url);
}
