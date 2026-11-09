use super::*;

#[test]
fn br_2_1_length_is_ten_to_one_hundred_twenty_eight_characters() {
    assert!(check("Abcdef1!gh").is_empty(), "exactly 10 passes");
    assert_eq!(
        check("Abcde1!gh"),
        vec![PasswordViolation::TooShort],
        "9 fails"
    );
    let longest = format!("Aa1!{}", "x".repeat(124));
    assert_eq!(longest.chars().count(), 128);
    assert!(check(&longest).is_empty(), "exactly 128 passes");
    let too_long = format!("Aa1!{}", "x".repeat(125));
    assert_eq!(check(&too_long), vec![PasswordViolation::TooLong]);
}

#[test]
fn br_2_1_length_counts_characters_not_bytes() {
    // Ten characters, more than ten bytes.
    assert!(check("Ünïcödé1!x").is_empty());
}

#[test]
fn br_2_2_requires_each_character_kind() {
    assert_eq!(
        check("abcdefghi1!"),
        vec![PasswordViolation::MissingUpperCase]
    );
    assert_eq!(
        check("ABCDEFGHI1!"),
        vec![PasswordViolation::MissingLowerCase]
    );
    assert_eq!(check("Abcdefghij!"), vec![PasswordViolation::MissingDigit]);
    assert_eq!(check("Abcdefghij1"), vec![PasswordViolation::MissingSymbol]);
}

#[test]
fn br_2_2_a_space_counts_as_a_symbol() {
    assert!(check("Correct horse 7").is_empty());
}

#[test]
fn br_2_6_reports_every_unmet_requirement_without_echoing_the_password() {
    let violations = check("abc");
    assert!(violations.contains(&PasswordViolation::TooShort));
    assert!(violations.contains(&PasswordViolation::MissingUpperCase));
    assert!(violations.contains(&PasswordViolation::MissingDigit));
    assert!(violations.contains(&PasswordViolation::MissingSymbol));
    assert!(violations.iter().all(|v| !v.requirement().contains("abc")));
}

#[test]
fn br_2_4_and_2_3_violations_have_plain_requirements() {
    assert!(
        PasswordViolation::SameAsCurrent
            .requirement()
            .contains("different")
    );
    assert!(
        PasswordViolation::Breached
            .requirement()
            .contains("breaches")
    );
}
