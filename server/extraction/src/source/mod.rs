use crate::models::error::SourceError;
use crate::models::source::{ImportSource, SourceKind};

mod blocks;
mod delimited;
mod json;
mod markup;
mod pdf;
mod sample;
mod spreadsheet;
mod structured;
pub(crate) mod text;

const PDF_SIGNATURE: &[u8] = b"%PDF-";
const PDF_SIGNATURE_WINDOW: usize = 1024;
const WORKBOOK_SIGNATURES: [&[u8]; 2] = [b"PK\x03\x04", b"\xD0\xCF\x11\xE0\xA1\xB1\x1A\xE1"];
const READABLE_IMAGE_SIGNATURES: [&[u8]; 3] = [b"\xFF\xD8\xFF", b"\x89PNG\r\n\x1A\n", b"GIF8"];
const OTHER_IMAGE_SIGNATURES: [&[u8]; 2] = [b"II*\x00", b"MM\x00*"];
const IMAGE_CONTAINER_BRANDS: [&[u8]; 6] = [b"heic", b"heix", b"mif1", b"msf1", b"avif", b"heif"];
const MAX_CONTROL_CHAR_RATIO: f64 = 0.01;

impl ImportSource {
    pub fn read(bytes: &[u8]) -> Result<Self, SourceError> {
        let source = if bytes[..bytes.len().min(PDF_SIGNATURE_WINDOW)]
            .windows(PDF_SIGNATURE.len())
            .any(|window| window == PDF_SIGNATURE)
        {
            pdf::read(bytes)?
        } else if WORKBOOK_SIGNATURES.iter().any(|s| bytes.starts_with(s)) {
            spreadsheet::read(bytes)?
        } else if is_readable_image(bytes) {
            return Ok(image());
        } else if is_other_image(bytes) {
            return Err(SourceError::ImageFormat);
        } else {
            let decoded = text::decode(bytes);
            if !is_mostly_text(&decoded) {
                return Err(SourceError::Unsupported);
            }
            if decoded.trim_start().starts_with(['{', '[']) && json::nests_too_deeply(&decoded) {
                return Err(SourceError::Unreadable(
                    "it is nested too deeply".to_string(),
                ));
            }
            text::read(&decoded)
        };

        if source.records.is_empty() && source.kind != SourceKind::Image {
            return Err(SourceError::Empty);
        }
        Ok(source)
    }
}

fn image() -> ImportSource {
    ImportSource {
        kind: SourceKind::Image,
        records: Vec::new(),
        notes: Vec::new(),
        pages_without_text: Vec::new(),
    }
}

fn is_readable_image(bytes: &[u8]) -> bool {
    READABLE_IMAGE_SIGNATURES
        .iter()
        .any(|s| bytes.starts_with(s))
        || (bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WEBP"))
}

fn is_other_image(bytes: &[u8]) -> bool {
    OTHER_IMAGE_SIGNATURES.iter().any(|s| bytes.starts_with(s))
        || (bytes.get(4..8) == Some(b"ftyp")
            && bytes
                .get(8..12)
                .is_some_and(|brand| IMAGE_CONTAINER_BRANDS.contains(&brand)))
}

fn is_mostly_text(text: &str) -> bool {
    let total = text.chars().count();
    let control = text
        .chars()
        .filter(|c| c.is_control() && !matches!(c, '\n' | '\r' | '\t' | '\u{c}'))
        .count();
    !text.contains('\0')
        && (total == 0 || (control as f64) / (total as f64) <= MAX_CONTROL_CHAR_RATIO)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn structured_formats_become_one_record_per_transaction() {
        for bytes in [
            &b"[{\"date\": \"2024-01-02\", \"amount\": \"-3.50\"}, {\"date\": \"2024-01-03\", \"amount\": \"4.00\"}]"[..],
            b"<Stmt><Ntry><Amt>3.50</Amt><Dt>2024-01-02</Dt></Ntry><Ntry><Amt>4.00</Amt><Dt>2024-01-03</Dt></Ntry></Stmt>",
            b"OFXHEADER:100\n<OFX><STMTTRN><DTPOSTED>20240102<TRNAMT>-3.50</STMTTRN><STMTTRN><DTPOSTED>20240103<TRNAMT>4.00</STMTTRN></OFX>",
            b"!Type:Bank\nD01/02/2024\nT-3.50\n^\nD01/03/2024\nT4.00\n^\n",
            b":20:STMT\n:61:2401020102D3,50NTRFNONREF\n:86:Coffee\n:61:2401030103C4,00NTRFNONREF\n:86:Refund\n",
        ] {
            let source = ImportSource::read(bytes).unwrap();
            let transactions: Vec<_> = source
                .records
                .iter()
                .filter(|record| record.fields.as_ref().is_some_and(|fields| fields.len() > 1))
                .collect();

            assert_eq!(source.kind, SourceKind::Text);
            assert_eq!(transactions.len(), 2, "{:#?}", source.records);
        }
    }

    #[test]
    fn json_nested_deeper_than_any_statement_is_unreadable() {
        let read_on_a_small_stack = |depth: usize| {
            let text = format!("{}{}", "[".repeat(depth), "]".repeat(depth));
            std::thread::Builder::new()
                .stack_size(2 * 1024 * 1024)
                .spawn(move || ImportSource::read(text.as_bytes()))
                .unwrap()
                .join()
                .unwrap()
        };

        assert!(matches!(
            read_on_a_small_stack(100_000),
            Err(SourceError::Unreadable(_))
        ));
        assert!(read_on_a_small_stack(100).is_ok());
    }

    #[test]
    fn an_image_is_read_from_the_picture_unless_its_format_is_one_providers_refuse() {
        for bytes in [
            &b"\xFF\xD8\xFF\xE0rest"[..],
            b"\x89PNG\r\n\x1A\nrest",
            b"RIFF\x00\x00\x00\x00WEBPVP8 ",
        ] {
            assert_eq!(ImportSource::read(bytes).unwrap().kind, SourceKind::Image);
        }
        for bytes in [&b"\x00\x00\x00\x18ftypheic\x00\x00"[..], b"II*\x00rest"] {
            assert_eq!(ImportSource::read(bytes), Err(SourceError::ImageFormat));
        }
    }

    #[test]
    fn binary_files_are_unsupported() {
        assert_eq!(
            ImportSource::read(b"\x7FELF\x02\x01\x01\x00\x00\x00\x00"),
            Err(SourceError::Unsupported)
        );
        assert_eq!(
            ImportSource::read(b"PK\x03\x04not really a workbook"),
            Err(SourceError::Unsupported)
        );
    }

    #[test]
    fn a_file_without_records_is_empty() {
        assert_eq!(ImportSource::read(b"\n \n"), Err(SourceError::Empty));
        assert_eq!(ImportSource::read(b""), Err(SourceError::Empty));
    }
}
