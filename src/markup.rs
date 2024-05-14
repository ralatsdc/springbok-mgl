use crate::{bill_section::BillSection, law_section::LawSectionWithText};
use fancy_regex::Regex;
use std::{error::Error, path::PathBuf};

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
    match_sections: Regex,
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
        strike_lines: Regex::new(r"strike_lines").unwrap(), //TODO: Implement
        strike_section: Regex::new(r"strike_section").unwrap(), //TODO: Implement
        insert_words: Regex::new(r#"insert.*word.*(“|")(.*)(”|").*.*?:-? (.*)\."#).unwrap(),
        insert_lines: Regex::new(r"insert_lines").unwrap(), //TODO: Implement
        insert_section: Regex::new(r"insert.*sections?:-?([\s\S]*)").unwrap(),
        match_sections: Regex::new(r"Section[\s\S]*?(?=Section|\z)").unwrap(),
    }
}
pub(crate) fn mark_section_text(
    law_section: &LawSectionWithText,
    bill_sections: &Vec<BillSection>,
    markup_regex: &MarkupRegex,
) -> Option<String> {
    // Parse law section title and contents
    if let Ok(Some(caps)) = markup_regex.text_parse.captures(law_section.text.as_ref()) {
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

fn mark_text(
    law_section_text: &String,
    bill_section_text: &String,
    bill_section_number: &String,
    markup_regex: &MarkupRegex,
) -> String {
    // Section amends an existing law
    let is_repealing = markup_regex.repealed.is_match(&*bill_section_text).unwrap();
    let is_striking = markup_regex.striking.is_match(&*bill_section_text).unwrap();
    let is_inserting = markup_regex
        .inserting
        .is_match(&*bill_section_text)
        .unwrap();
    let is_words = markup_regex.words.is_match(&*bill_section_text).unwrap();
    let is_sections = markup_regex.sections.is_match(&*bill_section_text).unwrap();
    let is_subsections = markup_regex
        .subsections
        .is_match(&*bill_section_text)
        .unwrap();
    let is_lines = markup_regex.lines.is_match(&*bill_section_text).unwrap();
    let mut marked_text = law_section_text.clone();

    // Repealing
    if is_repealing {
        if let Ok(Some(caps)) = markup_regex.repealed.captures(bill_section_text.as_ref()) {
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
            if let Ok(Some(caps)) = markup_regex
                .replace_words
                .captures(bill_section_text.as_ref())
            {
                let striked_words = String::from(&caps[2]);
                let inserted_words = String::from(&caps[4]);
                let mut buffer = "";

                let matches: Vec<&str> = law_section_text.matches(&striked_words).collect();
                // Replace word(s) only if one instance appears
                if matches.len() == 1 {
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
                } else {
                    println!(
                        "Replacing Words: ambiguous - bill section will be added as a footnote."
                    );
                    marked_text = format!("{}\n\n_{}_", law_section_text, bill_section_text.trim())
                }
            }
        }
        // Striking and inserting line(s)
        else if is_lines {
            // if let Ok(Some(caps)) = markup_regex
            //     .replace_lines
            //     .captures(bill_section_text.as_ref())
            // {
            //     let strike_start_line = String::from(&caps[1]);
            //     let strike_end_line = String::from(&caps[2]);
            //     let inserted_words = String::from(&caps[3]);
            //
            //     //TODO: figure out how to convert line numbers into actual strings
            //     let striked_words = String::from("PLACEHOLDER");
            //
            //     // Format replacement
            //     let replacement = format!(
            //         "\
            //     [.line-through .red]#{striked_words}# \
            //     [.blue]#{inserted_words}#^{bill_section_number}^\
            //     "
            //     );
            //
            //     marked_text = law_section_text.replace(&striked_words, &*replacement)
            // }
            println!("Replacing Line: line numbers are not included in the online version of the law, and thus cannot be accurately included. Bill section will be added as a footnote.");
            marked_text = format!("{}\n\n_{}_", law_section_text, bill_section_text.trim())
        }
        // Striking and inserting subsections(s)
        else if is_subsections {
            if let Ok(Some(caps)) = markup_regex
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
                if let Ok(Some(caps)) = get_subsection_regex.captures(law_section_text.as_ref()) {
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
            if let Ok(Some(caps)) = markup_regex
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
            if let Ok(Some(caps)) = markup_regex
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
            println!("Striking Line: line numbers are not included in the online version of the law, and thus cannot be accurately included. Bill section will be added as a footnote.");
            marked_text = format!("{}\n\n_{}_", law_section_text, bill_section_text.trim())
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
            println!("Inserting Words (at line): line numbers are not included in the online version of the law, and thus cannot be accurately included. Bill section will be added as a footnote.");
            marked_text = format!("{}\n\n_{}_", law_section_text, bill_section_text.trim())
        }
        // Inserting line(s)
        else if is_lines {
            println!("Inserting Line: line numbers are not included in the online version of the law, and thus cannot be accurately included. Bill section will be added as a footnote.");
            marked_text = format!("{}\n\n_{}_", law_section_text, bill_section_text.trim())
        }
        // Inserting section(s)
        else if is_sections {
            if let Ok(Some(caps)) = markup_regex
                .insert_section
                .captures(bill_section_text.as_ref())
            {
                let section_text = String::from(caps[1].trim());
                let matches: Vec<_> = markup_regex
                    .match_sections
                    .find_iter(&section_text)
                    .map(|m| m.expect("BAD REGEX").as_str().trim())
                    .collect();
                let insert = matches.join("#\n\n[.blue]#");
                // Format replacement
                marked_text = format!(
                    "\
                        {law_section_text}\n\n[.blue]#{insert}#^{bill_section_number}^\
                        "
                )
            }
        }
    } else {
        println!("Not sure what section does: {}", &*law_section_text);
    }
    marked_text
}

pub(crate) fn get_adoc_paths(dir: &str) -> Result<Vec<PathBuf>, Box<dyn Error>> {
    let paths = std::fs::read_dir(dir)?
        // Filter out all those directory entries which couldn't be read
        .filter_map(|res| res.ok())
        // Map the directory entries to paths
        .map(|dir_entry| dir_entry.path())
        // Filter out all paths with extensions other than `csv`
        .filter_map(|path| {
            if path.extension().map_or(false, |ext| ext == "adoc") {
                Some(path)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    Ok(paths)
}
