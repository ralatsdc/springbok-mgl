use clap::{Parser, ValueEnum};

// See:
// - https://docs.rs/clap/latest/clap/_derive/_tutorial/chapter_0/index.html#
// - https://docs.rs/clap/latest/clap/_derive/_tutorial/chapter_3/index.html

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum Branch {
    House,
    Senate,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum DocumentType {
    Amendment,
    Bill,
    Communication,
    Order,
    Report,
    Resolve,
}

/// Produce strikethrough and underline markup for a bill before the Massachusetts legislature
#[derive(Parser)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// Search for legislation using this search term
    #[arg(short = 's', long)]
    pub search_term: Option<String>,

    /// Identify legislation from this legislative session
    #[arg(short = 'c', long)]
    general_court: Option<String>,

    /// Include legislation in this branch of the legislature
    #[arg(short = 'b', long, value_enum)]
    branch: Option<Branch>,

    /// Include legislation sponsored by this legislator
    #[arg(short = 'l', long)]
    sponsor_legislator: Option<String>,

    /// Include legislation sponsored by this legislative committee
    #[arg(short = 'm', long)]
    sponsor_committee: Option<String>,

    /// Include legislation sponsored by a governor, or state organization
    #[arg(short = 'o', long)]
    sponsor_other: Option<String>,

    /// Identify legislation of this document type
    #[arg(short = 'd', long, value_enum)]
    document_type: Option<DocumentType>,
}
