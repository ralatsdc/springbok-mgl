use springbok_mgl;
use springbok_mgl::{
    collect_bill_sections, count_bill_section_types, init_section_regex, SectionCounts,
};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

fn nodes_from_file(filename: impl AsRef<Path>) -> Vec<String> {
    let file = File::open(filename).expect("no such file");
    let buf = BufReader::new(file);
    buf.lines()
        .map(|l| l.expect("Could not parse line"))
        .collect()
}

fn assert_section_counts(section_counts: SectionCounts, expected_section_counts: SectionCounts) {
    assert_eq!(
        section_counts.total, expected_section_counts.total,
        "Expected section count total was '{}', actual result was '{}'",
        expected_section_counts.total, section_counts.total
    );
    assert_eq!(
        section_counts.amending, expected_section_counts.amending,
        "Expected section count amending was '{}', actual result was '{}'",
        expected_section_counts.amending, section_counts.amending
    );
    assert_eq!(
        section_counts.amending_by_striking_and_inserting, expected_section_counts.amending_by_striking_and_inserting,
        "Expected section count amending_by_striking_and_inserting was '{}', actual result was '{}'",
        expected_section_counts.amending_by_striking_and_inserting, section_counts.amending_by_striking_and_inserting
    );
    assert_eq!(
        section_counts.amending_by_striking, expected_section_counts.amending_by_striking,
        "Expected section count amending_by_striking was '{}', actual result was '{}'",
        expected_section_counts.amending_by_striking, section_counts.amending_by_striking
    );
    assert_eq!(
        section_counts.amending_by_inserting, expected_section_counts.amending_by_inserting,
        "Expected section count amending_by_inserting was '{}', actual result was '{}'",
        expected_section_counts.amending_by_inserting, section_counts.amending_by_inserting
    );
    assert_eq!(
        section_counts.repealing, expected_section_counts.repealing,
        "Expected section count repealing was '{}', actual result was '{}'",
        expected_section_counts.repealing, section_counts.repealing
    );
    assert_eq!(
        section_counts.other, expected_section_counts.other,
        "Expected section count other was '{}', actual result was '{}'",
        expected_section_counts.other, section_counts.other
    );
    assert_eq!(
        section_counts.amending,
        section_counts.amending_by_striking
            + section_counts.amending_by_inserting
            + section_counts.amending_by_striking_and_inserting
    );
    assert_eq!(
        section_counts.total,
        section_counts.amending + section_counts.repealing + section_counts.other
    );
}

#[test]
fn it_counts_hd4607() {
    let expected_section_counts = SectionCounts {
        total: 140,
        amending: 127,
        amending_by_striking_and_inserting: 101,
        amending_by_striking: 17,
        amending_by_inserting: 9,
        repealing: 3,
        other: 10,
    };
    let text_nodes = nodes_from_file("./tests/test-data/HD.4607.txt");
    let section_regex = init_section_regex();
    let bill = collect_bill_sections(text_nodes, &section_regex);
    let section_counts = count_bill_section_types(&bill, &section_regex);
    assert_section_counts(section_counts, expected_section_counts);
}

#[test]
fn it_counts_h4072() {
    let expected_section_counts = SectionCounts {
        total: 3,
        amending: 0,
        amending_by_striking_and_inserting: 0,
        amending_by_striking: 0,
        amending_by_inserting: 0,
        repealing: 0,
        other: 3,
    };
    let text_nodes = nodes_from_file("./tests/test-data/H.4072.txt");
    let section_regex = init_section_regex();
    let bill = collect_bill_sections(text_nodes, &section_regex);
    let section_counts = count_bill_section_types(&bill, &section_regex);
    assert_section_counts(section_counts, expected_section_counts);
}

#[ignore]
#[test]
fn it_counts_h4072_lower() {
    let expected_section_counts = SectionCounts {
        total: 3,
        amending: 0,
        amending_by_striking_and_inserting: 0,
        amending_by_striking: 0,
        amending_by_inserting: 0,
        repealing: 0,
        other: 3,
    };
    let text_nodes = nodes_from_file("./tests/test-data/H.4072.lower.txt");
    let section_regex = init_section_regex();
    let bill = collect_bill_sections(text_nodes, &section_regex);
    let section_counts = count_bill_section_types(&bill, &section_regex);
    assert_section_counts(section_counts, expected_section_counts);
}

#[test]
fn it_counts_s2482() {
    let expected_section_counts = SectionCounts {
        total: 4,
        amending: 2,
        amending_by_striking_and_inserting: 0,
        amending_by_striking: 1,
        amending_by_inserting: 1,
        repealing: 0,
        other: 2,
    };
    let text_nodes = nodes_from_file("./tests/test-data/S.2482.txt");
    let section_regex = init_section_regex();
    let bill = collect_bill_sections(text_nodes, &section_regex);
    let section_counts = count_bill_section_types(&bill, &section_regex);
    assert_section_counts(section_counts, expected_section_counts);
}

#[test]
fn it_counts_h4220() {
    let expected_section_counts = SectionCounts {
        total: 7,
        amending: 6,
        amending_by_striking_and_inserting: 6,
        amending_by_striking: 0,
        amending_by_inserting: 0,
        repealing: 0,
        other: 1,
    };
    let text_nodes = nodes_from_file("./tests/test-data/H.4220.txt");
    let section_regex = init_section_regex();
    let bill = collect_bill_sections(text_nodes, &section_regex);
    let section_counts = count_bill_section_types(&bill, &section_regex);
    assert_section_counts(section_counts, expected_section_counts);
}

#[test]
fn it_counts_sd2897() {
    let expected_section_counts = SectionCounts {
        total: 1,
        amending: 1,
        amending_by_striking_and_inserting: 0,
        amending_by_striking: 0,
        amending_by_inserting: 1,
        repealing: 0,
        other: 0,
    };
    let text_nodes = nodes_from_file("./tests/test-data/SD.2897.txt");
    let section_regex = init_section_regex();
    let bill = collect_bill_sections(text_nodes, &section_regex);
    let section_counts = count_bill_section_types(&bill, &section_regex);
    assert_section_counts(section_counts, expected_section_counts);
}

#[test]
fn it_counts_hd4741() {
    let expected_section_counts = SectionCounts {
        total: 1,
        amending: 0,
        amending_by_striking_and_inserting: 0,
        amending_by_striking: 0,
        amending_by_inserting: 0,
        repealing: 0,
        other: 1,
    };
    let text_nodes = nodes_from_file("./tests/test-data/HD.4741.txt");
    let section_regex = init_section_regex();
    let bill = collect_bill_sections(text_nodes, &section_regex);
    let section_counts = count_bill_section_types(&bill, &section_regex);
    assert_section_counts(section_counts, expected_section_counts);
}

#[test]
fn it_counts_h47() {
    let expected_section_counts = SectionCounts {
        total: 4,
        amending: 1,
        amending_by_striking_and_inserting: 1,
        amending_by_striking: 0,
        amending_by_inserting: 0,
        repealing: 0,
        other: 3,
    };
    let text_nodes = nodes_from_file("./tests/test-data/H.47.txt");
    let section_regex = init_section_regex();
    let bill = collect_bill_sections(text_nodes, &section_regex);
    let section_counts = count_bill_section_types(&bill, &section_regex);
    assert_section_counts(section_counts, expected_section_counts);
}

#[ignore]
#[test]
fn it_counts_h47_lower() {
    let expected_section_counts = SectionCounts {
        total: 4,
        amending: 1,
        amending_by_striking_and_inserting: 1,
        amending_by_striking: 0,
        amending_by_inserting: 0,
        repealing: 0,
        other: 3,
    };
    let text_nodes = nodes_from_file("./tests/test-data/H.47.lower.txt");
    let section_regex = init_section_regex();
    let bill = collect_bill_sections(text_nodes, &section_regex);
    let section_counts = count_bill_section_types(&bill, &section_regex);
    assert_section_counts(section_counts, expected_section_counts);
}
