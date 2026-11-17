use chardetng::{EncodingDetector, Iso2022JpDetection, Utf8Detection};
use encoding_rs::{Encoding, UTF_16BE, UTF_16LE};

use super::{delimited, structured};
use crate::models::source::{ImportSource, SourceKind, SourceLocation, SourceRecord};

pub fn decode(bytes: &[u8]) -> String {
    if let Some((encoding, bom_length)) = Encoding::for_bom(bytes) {
        let (text, _) = encoding.decode_without_bom_handling(&bytes[bom_length..]);
        return text.into_owned();
    }
    let nuls_at = |offset: usize| {
        bytes
            .iter()
            .skip(offset)
            .step_by(2)
            .filter(|byte| **byte == 0)
            .count()
    };
    let (little, big) = (nuls_at(1), nuls_at(0));
    let line_breaks = bytes.iter().filter(|byte| **byte == b'\n').count();
    if little.max(big) > 0 && little.max(big) >= line_breaks {
        let encoding = if little >= big { UTF_16LE } else { UTF_16BE };
        let (text, had_errors) = encoding.decode_without_bom_handling(bytes);
        if !had_errors && !text.contains('\0') {
            return text.into_owned();
        }
    }
    if let Ok(text) = std::str::from_utf8(bytes) {
        return text.to_string();
    }
    let mut detector = EncodingDetector::new(Iso2022JpDetection::Deny);
    detector.feed(bytes, true);
    let (text, _) = detector
        .guess(None, Utf8Detection::Deny)
        .decode_without_bom_handling(bytes);
    text.into_owned()
}

pub fn read(text: &str) -> ImportSource {
    let text = text.replace("\r\n", "\n").replace('\r', "\n");
    structured::read(&text)
        .or_else(|| delimited::tokenize(&text))
        .unwrap_or_else(|| ImportSource {
            kind: SourceKind::Text,
            records: text
                .lines()
                .enumerate()
                .filter(|(_, line)| !line.trim().is_empty())
                .map(|(index, line)| SourceRecord {
                    location: SourceLocation::Lines {
                        first: index as u32 + 1,
                        last: index as u32 + 1,
                    },
                    text: line.trim_end().to_string(),
                    fields: None,
                })
                .collect(),
            notes: Vec::new(),
            pages_without_text: Vec::new(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn utf8_with_bom_drops_the_bom() {
        assert_eq!(decode(b"\xEF\xBB\xBFdate,amount"), "date,amount");
    }

    #[test]
    fn utf16_without_a_byte_order_mark_is_decoded() {
        let text = "Data;Suma\r\n2024-01-02;-3,50;Ādaži\r\n";
        let little: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
        let big: Vec<u8> = text.encode_utf16().flat_map(u16::to_be_bytes).collect();

        assert_eq!(decode(&little), text);
        assert_eq!(decode(&big), text);
    }

    #[test]
    fn text_with_a_stray_nul_is_not_taken_for_utf16() {
        let bytes =
            b"Date,Description,Amount\n2024-01-02,Coffee,-3.50\n2024-01-03,Rents\0,-500.00\n";
        assert_eq!(bytes.len() % 2, 0);

        assert!(decode(bytes).contains("Coffee"));
        assert!(decode(bytes).contains('\0'));
    }

    #[test]
    fn legacy_single_byte_encodings_are_detected() {
        for (encoding, text) in [
            (
                encoding_rs::WINDOWS_1257,
                "Gavėjas;Paaiškinimai;Suma\nUAB Maxima;Mokėjimas už prekes;12,50\n",
            ),
            (
                encoding_rs::WINDOWS_1251,
                "Дата;Описание;Сумма\n02.01.2024;Оплата в магазине продуктов;-150,00\n",
            ),
            (
                encoding_rs::WINDOWS_1252,
                "Datum;Beschreibung;Betrag\n02.01.2024;Überweisung an Müller für Gebühren;-12,50\n",
            ),
        ] {
            let (bytes, _, _) = encoding.encode(text);
            assert_eq!(decode(&bytes), text, "{}", encoding.name());
        }
    }

    #[test]
    fn free_text_becomes_one_record_per_non_blank_line() {
        let source = read("coffee 3.50 on 2 March\n\n  \nrent 1200 on the 1st\r\n");

        assert_eq!(source.kind, SourceKind::Text);
        assert_eq!(source.records.len(), 2);
        assert_eq!(source.records[0].text, "coffee 3.50 on 2 March");
        assert_eq!(
            source.records[1].location,
            SourceLocation::Lines { first: 4, last: 4 }
        );
        assert_eq!(source.records[1].fields, None);
    }

    #[test]
    fn old_mac_line_endings_split_lines() {
        let source = read("first line\rsecond line\r");

        assert_eq!(source.records.len(), 2);
    }
}
