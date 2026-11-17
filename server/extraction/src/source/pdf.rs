use pdf_oxide::layout::Word;
use pdf_oxide::PdfDocument;

use crate::models::error::SourceError;
use crate::models::source::{ImportSource, SourceKind, SourceLocation, SourceRecord};

const MIN_TEXT_CHARS_PER_PAGE: usize = 20;
const WORD_GAP_IN_CHARS: f32 = 1.5;
const MAX_LINE_WIDTH: usize = 1000;

pub fn read(bytes: &[u8]) -> Result<ImportSource, SourceError> {
    let document = PdfDocument::from_bytes(bytes.to_vec()).map_err(source_error)?;
    if !document.is_authenticated() {
        return Err(SourceError::Encrypted);
    }
    let pages = document.page_count().map_err(source_error)?;

    let mut records = Vec::new();
    let mut pages_without_text = Vec::new();
    for page in 0..pages {
        let words = document.extract_words(page).map_err(source_error)?;
        let lines = layout_lines(words);
        let text_chars: usize = lines
            .iter()
            .map(|line| line.chars().filter(|c| c.is_alphanumeric()).count())
            .sum();
        if text_chars < MIN_TEXT_CHARS_PER_PAGE {
            pages_without_text.push(page as u32 + 1);
        }
        for (line, text) in lines.into_iter().enumerate() {
            records.push(SourceRecord {
                location: SourceLocation::Page {
                    page: page as u32 + 1,
                    line: line as u32 + 1,
                },
                fields: Some(
                    text.split("  ")
                        .map(str::trim)
                        .filter(|field| !field.is_empty())
                        .map(str::to_string)
                        .collect(),
                ),
                text,
            });
        }
    }

    if pages > 0 && pages_without_text.len() == pages {
        return Ok(super::image());
    }

    let mut notes = vec![
        "PDF text laid out by position: one record per visual line, each carrying its `page`, columns kept apart by spacing; `fields` (strings) split a line at gaps of two or more spaces and `columns` holds where each starts in `text`; this sample shows each field as @column value"
            .to_string(),
    ];
    if !pages_without_text.is_empty() {
        notes.push(format!(
            "pages {:?} have no text layer (scanned?); their contents are not in these records",
            pages_without_text
        ));
    }
    Ok(ImportSource {
        kind: SourceKind::Pdf,
        records,
        notes,
        pages_without_text,
    })
}

fn source_error(error: pdf_oxide::Error) -> SourceError {
    match error {
        pdf_oxide::Error::EncryptedPdf => SourceError::Encrypted,
        other => SourceError::Unreadable(other.to_string()),
    }
}

fn layout_lines(mut words: Vec<Word>) -> Vec<String> {
    words.retain(|w| !w.text.trim().is_empty());
    let mut char_widths: Vec<f32> = words
        .iter()
        .map(|w| w.bbox.width / w.text.chars().count() as f32)
        .filter(|width| width.is_finite() && *width > 0.0)
        .collect();
    char_widths.sort_by(f32::total_cmp);
    let Some(&char_width) = char_widths.get(char_widths.len() / 2) else {
        return Vec::new();
    };

    let center = |w: &Word| w.bbox.y + w.bbox.height / 2.0;
    words.sort_by(|a, b| center(b).total_cmp(&center(a)));

    let mut rows: Vec<(f32, f32, Vec<Word>)> = Vec::new();
    for word in words {
        let word_center = center(&word);
        match rows.last_mut() {
            Some((row_center, row_height, row))
                if (word_center - *row_center).abs() <= row_height.max(word.bbox.height) / 2.0 =>
            {
                row.push(word)
            }
            _ => rows.push((word_center, word.bbox.height, vec![word])),
        }
    }

    rows.into_iter()
        .map(|(_, _, mut row)| {
            row.sort_by(|a, b| a.bbox.x.total_cmp(&b.bbox.x));
            let mut line = String::new();
            let mut previous_end: Option<f32> = None;
            for word in row {
                let column =
                    ((word.bbox.x / char_width).round().max(0.0) as usize).min(MAX_LINE_WIDTH);
                let length = line.chars().count();
                match previous_end {
                    Some(end) if word.bbox.x - end < char_width * WORD_GAP_IN_CHARS => {
                        line.push(' ')
                    }
                    _ if column > length => line.push_str(&" ".repeat(column - length)),
                    _ if length > 0 => line.push_str("  "),
                    _ => {}
                }
                line.push_str(&word.text);
                previous_end = Some(word.bbox.x + word.bbox.width);
            }
            line.trim_end().to_string()
        })
        .filter(|line| !line.is_empty())
        .collect()
}

#[cfg(test)]
mod tests {
    use pdf_oxide::editor::{EncryptionAlgorithm, EncryptionConfig, Permissions};
    use pdf_oxide::writer::DocumentBuilder;

    use super::*;

    fn statement_pdf(extra_blank_page: bool) -> Vec<u8> {
        let mut builder = DocumentBuilder::new();
        builder
            .a4_page()
            .font("Helvetica", 10.0)
            .at(50.0, 780.0)
            .text("Account statement for the period 2023-12-01 to 2023-12-31")
            .at(50.0, 740.0)
            .text("Date")
            .at(140.0, 740.0)
            .text("Description")
            .at(330.0, 740.0)
            .text("Money out")
            .at(420.0, 740.0)
            .text("Money in")
            .at(500.0, 740.0)
            .text("Balance")
            .at(50.0, 720.0)
            .text("14 Dec 2023")
            .at(140.0, 720.0)
            .text("Corner Bakery Co")
            .at(330.0, 720.0)
            .text("£47.18")
            .at(500.0, 720.0)
            .text("£3,418.27")
            .at(140.0, 708.0)
            .text("To: Corner Bakery Co, Leeds")
            .at(50.0, 690.0)
            .text("14 Dec 2023")
            .at(140.0, 690.0)
            .text("Payment from Employer")
            .at(420.0, 690.0)
            .text("£1,286.40")
            .at(500.0, 690.0)
            .text("£4,704.67")
            .done();
        if extra_blank_page {
            builder.a4_page().done();
        }
        builder.build().unwrap()
    }

    #[test]
    fn rows_are_read_top_to_bottom_with_columns_apart() {
        let source = read(&statement_pdf(false)).unwrap();
        let lines: Vec<&str> = source.records.iter().map(|r| r.text.as_str()).collect();

        assert_eq!(source.kind, SourceKind::Pdf);
        assert_eq!(lines.len(), 5, "{lines:#?}");
        assert!(lines[0].contains("Account statement for the period 2023-12-01 to 2023-12-31"));
        assert!(lines[3].contains("To: Corner Bakery Co, Leeds"));
        assert_eq!(
            source.records[4].location,
            SourceLocation::Page { page: 1, line: 5 }
        );
        assert!(source.pages_without_text.is_empty());
    }

    #[test]
    fn money_out_and_money_in_land_in_different_columns() {
        let source = read(&statement_pdf(false)).unwrap();
        let header = &source.records[1].text;
        let spent = &source.records[2].text;
        let received = &source.records[4].text;

        let out_column = header.find("Money out").unwrap();
        let in_column = header.find("Money in").unwrap();
        let spent_at = spent.find("£47.18").unwrap();
        let received_at = received.find("£1,286.40").unwrap();
        assert!(
            spent_at.abs_diff(out_column) < in_column.abs_diff(spent_at),
            "{header}\n{spent}"
        );
        assert!(
            received_at.abs_diff(in_column) < received_at.abs_diff(out_column),
            "{header}\n{received}"
        );
    }

    #[test]
    fn pages_without_text_are_reported_and_a_document_with_none_is_read_as_an_image() {
        let source = read(&statement_pdf(true)).unwrap();
        assert_eq!(source.pages_without_text, vec![2]);
        assert!(source.notes[1].starts_with("pages [2] have no text layer"));

        let mut builder = DocumentBuilder::new();
        builder.a4_page().done();
        assert_eq!(
            read(&builder.build().unwrap()).unwrap().kind,
            SourceKind::Image
        );
    }

    fn locked_pdf(algorithm: EncryptionAlgorithm, user_password: &str) -> Vec<u8> {
        let mut builder = DocumentBuilder::new();
        builder
            .a4_page()
            .font("Helvetica", 10.0)
            .at(50.0, 780.0)
            .text("Account statement 01/01/2024 to 31/01/2024")
            .at(50.0, 740.0)
            .text("02/01/2024")
            .at(140.0, 740.0)
            .text("Coffee shop")
            .at(420.0, 740.0)
            .text("3.50")
            .done();
        builder
            .to_bytes_with_encryption(EncryptionConfig {
                user_password: user_password.to_string(),
                owner_password: "bank-secret".to_string(),
                algorithm,
                permissions: Permissions::read_only(),
            })
            .unwrap()
    }

    #[test]
    fn a_pdf_locked_only_against_editing_reads_normally() {
        for algorithm in [
            EncryptionAlgorithm::Rc4_40,
            EncryptionAlgorithm::Rc4_128,
            EncryptionAlgorithm::Aes128,
            EncryptionAlgorithm::Aes256,
        ] {
            let source = read(&locked_pdf(algorithm, "")).unwrap();
            let lines: Vec<&str> = source.records.iter().map(|r| r.text.as_str()).collect();

            assert_eq!(lines.len(), 2, "{algorithm:?} {lines:#?}");
            assert!(lines[0].contains("Account statement 01/01/2024 to 31/01/2024"));
            assert_eq!(
                source.records[1].fields.as_ref().unwrap(),
                &vec!["02/01/2024", "Coffee shop", "3.50"]
            );
        }
    }

    #[test]
    fn a_pdf_that_needs_a_password_is_reported_as_encrypted() {
        for algorithm in [EncryptionAlgorithm::Rc4_128, EncryptionAlgorithm::Aes256] {
            assert_eq!(
                read(&locked_pdf(algorithm, "open-sesame")),
                Err(SourceError::Encrypted),
                "{algorithm:?}"
            );
        }
    }
}
