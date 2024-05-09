use crate::law_section::{collect_law_sections, LawSections};
use log::info;
use regex::Regex;
use scraper::{Html, Selector};
use url::Url;

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

#[derive(Debug, Clone)]
pub struct BillSectionRegex {
    bill_section: Regex,
    amended: Regex,
    striking: Regex,
    inserting: Regex,
    repealed: Regex,
}

// TODO: Document these?
pub fn init_bill_section_regex() -> BillSectionRegex {
    BillSectionRegex {
        bill_section: Regex::new(r"^\s*SECTION\s*(\d*\w*)\s*\.").unwrap(),
        amended: Regex::new(r"amended").unwrap(),
        striking: Regex::new(r"striking").unwrap(),
        inserting: Regex::new(r"inserting").unwrap(),
        repealed: Regex::new(r"repealed").unwrap(),
    }
}
#[derive(Debug)]
pub struct BillSection {
    pub section_number: String,
    pub text: String,
    pub law_sections: LawSections,
}
pub fn collect_bill_sections(
    text_nodes: &Vec<String>,
    section_regex: &BillSectionRegex,
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
    section_regex: &BillSectionRegex,
    bill: &mut Vec<BillSection>,
) {
    let section_str = section_text.as_str();
    let mut section_number = String::from("");
    if let Some(caps) = section_regex.bill_section.captures(section_str) {
        section_number = String::from(&caps[1]);
    } else {
        println!("{section_str}");
    }
    let law_sections = collect_law_sections(&section_number, section_str);
    let bill_section = BillSection {
        section_number,
        text: section_text.to_string(),
        law_sections,
    };
    bill.push(bill_section)
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
pub fn count_bill_section_types(
    bill: &Vec<BillSection>,
    section_regex: &BillSectionRegex,
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
    section_counts
}
pub fn print_bill_section_types(section_counts: SectionCounts) -> () {
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
}
