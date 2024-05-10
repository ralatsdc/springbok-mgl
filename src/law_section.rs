use crate::bill_section::{BillSection, BillSectionRegex};
use fancy_regex::Regex;
use log::info;
use scraper::{Element, Html, Selector};
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::sync::mpsc;
use std::sync::mpsc::Sender;
use std::thread;
use url::Url;

pub fn get_section_key(chapter: &String, section: &String) -> String {
    String::from(chapter.to_string() + "-" + section)
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
pub struct LawSectionWithText {
    pub(crate) law_chapter_key: String,
    pub(crate) text: String,
    pub(crate) bill_section_keys: Vec<String>,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct LawSections {
    pub chapter_number: String,
    pub section_numbers: Vec<String>,
}
#[derive(Debug, Clone)]
pub struct LawSectionRegex {
    law_chapter: Regex,
    law_section: Regex,
    section_list: Regex,
}

pub fn init_law_section_regex() -> LawSectionRegex {
    LawSectionRegex {
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
pub fn collect_law_sections(_bill_section_number: &str, section_str: &str) -> LawSections {
    // Init section regex
    let law_section_regex = init_law_section_regex();
    // Capture law chapter
    let mut law_chapter = String::from("");
    if let Some(caps) = law_section_regex.law_chapter.captures(section_str).unwrap() {
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
    if let Some(caps) = law_section_regex.law_section.captures(section_str).unwrap() {
        if caps[1].trim().to_lowercase().eq("section") {
            // Found a single section
            law_sections.push(String::from((&caps[2]).trim_end()));
        } else if caps[1].trim().to_lowercase().eq("sections") {
            // Found multiple, comma delimited sections
            let mut sections: Vec<_> = law_section_regex
                .section_list
                .find_iter(section_str)
                .map(|m| m.expect("Bad Regex").as_str())
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
