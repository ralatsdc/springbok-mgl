use clap::Parser;
use indexmap::IndexMap;
use log::info;

use springbok_mgl::*;

fn main() {
    env_logger::init();

    // Parse command line arguments and construct search URL
    info!("Constructing search URL");
    let cli = Cli::parse();
    let (do_search, search_url, search_term) = get_search_page(&cli);

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

            // Collect bill sections and law sections into structs with regex
            let section_regex = init_section_regex();
            let bill = collect_bill_sections(text_nodes, &section_regex);
            // Count type of bill sections with regex
            let _section_counts = count_bill_section_types(&bill, &section_regex);

            let _law_sections_text = get_required_law_sections(&bill);
        } else {
            info!("Search term is not a bill number")
        }
    }
}
