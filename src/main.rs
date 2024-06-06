use clap::Parser;
use indexmap::IndexMap;
use log::info;
use springbok_mgl::*;
use std::string::String;

fn main() {
    // Init logger
    env_logger::init();

    // Parse CLI
    let cli = Cli::parse();

    // Get search results in map and search_term
    let (search_results_map, search_term) = create_search_results_map(&cli);

    if cli.download {
        // Get and print bill text when searching by bill number
        if let Some(&ref search_entry) = search_results_map.get(search_term.as_str()).as_deref() {
            // Create bill struct
            let bill = create_bill(search_entry);

            // Create markup documents when output_filename specified
            if let Some(output_filename) = cli.output_filename {
                // Download all referenced law sections from bill
                let law_sections_text = create_law_sections_text(&bill);

                // Write the bill text to a file
                let output_folder = search_term;
                write_bill(&bill, &output_filename, &output_folder);

                // Write laws with bill proposed modifications in asciidoc format
                let law_folder = "modified-laws";
                write_asciidocs(law_sections_text, &bill, &output_folder, law_folder);

                // Run asciidoctor over newly created .adoc files
                run_asciidoctor(output_folder);
            }
        } else {
            info!("Search term is not a bill number")
        }
    }
}
