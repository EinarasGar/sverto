use std::io::Cursor;

use calamine::{open_workbook_auto_from_rs, Data, Reader};

use crate::models::error::SourceError;
use crate::models::source::{ImportSource, SourceKind, SourceLocation, SourceRecord};

const FLOAT_DECIMALS: usize = 10;

pub fn read(bytes: &[u8]) -> Result<ImportSource, SourceError> {
    let mut workbook =
        open_workbook_auto_from_rs(Cursor::new(bytes)).map_err(|_| SourceError::Unsupported)?;

    let mut records = Vec::new();
    let mut sheets = Vec::new();
    for sheet in workbook.sheet_names() {
        let range = workbook
            .worksheet_range(&sheet)
            .map_err(|e| SourceError::Unreadable(e.to_string()))?;
        let first_row = range.start().map_or(0, |(row, _)| row);
        let first_record = records.len();
        for (offset, row) in range.rows().enumerate() {
            let mut fields: Vec<String> = row.iter().map(render_cell).collect();
            while fields.last().is_some_and(|f| f.trim().is_empty()) {
                fields.pop();
            }
            if fields.is_empty() {
                continue;
            }
            records.push(SourceRecord {
                location: SourceLocation::Row {
                    sheet: sheet.clone(),
                    row: first_row + offset as u32 + 1,
                },
                text: fields.join("\t"),
                fields: Some(fields),
            });
        }
        if records.len() > first_record {
            sheets.push(format!(
                "{sheet:?} at refs {first_record}–{}",
                records.len() - 1
            ));
        }
    }

    Ok(ImportSource {
        kind: SourceKind::Spreadsheet,
        records,
        notes: vec![
            "spreadsheet cells, one record per non-empty row, each carrying its `sheet` and `row`; fields start at each sheet's first used column; dates are ISO, numbers unformatted"
                .to_string(),
            format!("sheets: {}", sheets.join(", ")),
        ],
        pages_without_text: Vec::new(),
    })
}

fn render_cell(cell: &Data) -> String {
    match cell {
        Data::Empty => String::new(),
        Data::String(s) | Data::DurationIso(s) => s.trim().to_string(),
        Data::DateTimeIso(s) => s.trim().trim_end_matches("T00:00:00").to_string(),
        Data::Int(i) => i.to_string(),
        Data::Float(f) => render_float(*f),
        Data::Bool(b) => b.to_string(),
        Data::DateTime(value) => match value.as_datetime() {
            Some(datetime) if (0.0..1.0).contains(&value.as_f64()) => datetime.time().to_string(),
            Some(datetime) if value.as_f64().fract() == 0.0 => datetime.date().to_string(),
            Some(datetime) => datetime.to_string(),
            None => render_float(value.as_f64()),
        },
        Data::Error(error) => error.to_string(),
    }
}

fn render_float(value: f64) -> String {
    let rounded = format!("{:.*}", FLOAT_DECIMALS, value);
    let trimmed = rounded.trim_end_matches('0').trim_end_matches('.');
    match trimmed {
        "-0" | "" => "0".to_string(),
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use rust_xlsxwriter::{ExcelDateTime, Format, Workbook};

    use super::*;

    fn workbook_bytes() -> Vec<u8> {
        let mut workbook = Workbook::new();
        let date_format = Format::new().set_num_format("dd/mm/yyyy");
        let sheet = workbook.add_worksheet().set_name("Statement").unwrap();
        sheet.write_string(0, 0, "Date").unwrap();
        sheet.write_string(0, 1, "Description, with comma").unwrap();
        sheet.write_string(0, 2, "Amount").unwrap();
        sheet
            .write_datetime_with_format(
                1,
                0,
                ExcelDateTime::from_ymd(2024, 1, 2).unwrap(),
                &date_format,
            )
            .unwrap();
        sheet.write_string(1, 1, "Coffee; shop").unwrap();
        sheet.write_number(1, 2, -12.3).unwrap();
        sheet.write_number(3, 2, 1500).unwrap();
        sheet.write_number(4, 2, 1234.5600000000002).unwrap();
        sheet.write_number(5, 2, 0.00012345).unwrap();
        sheet.write_number(6, 2, 0.1 + 0.2).unwrap();
        let other = workbook.add_worksheet().set_name("Notes").unwrap();
        other.write_string(2, 1, "Closing balance").unwrap();
        workbook.save_to_buffer().unwrap()
    }

    #[test]
    fn rows_become_records_with_cells_kept_intact() {
        let source = read(&workbook_bytes()).unwrap();

        assert_eq!(source.kind, SourceKind::Spreadsheet);
        assert_eq!(
            source.records[0].fields.as_ref().unwrap()[1],
            "Description, with comma"
        );
        assert_eq!(
            source.records[1].fields.as_ref().unwrap(),
            &vec!["2024-01-02", "Coffee; shop", "-12.3"]
        );
    }

    #[test]
    fn float_noise_is_removed_without_losing_small_quantities() {
        let source = read(&workbook_bytes()).unwrap();
        let amounts: Vec<&str> = source.records[2..6]
            .iter()
            .map(|r| r.fields.as_ref().unwrap()[2].as_str())
            .collect();

        assert_eq!(amounts, vec!["1500", "1234.56", "0.00012345", "0.3"]);
    }

    #[test]
    fn empty_rows_are_skipped_and_locations_keep_sheet_rows() {
        let source = read(&workbook_bytes()).unwrap();

        assert_eq!(
            source.records[2].location,
            SourceLocation::Row {
                sheet: "Statement".to_string(),
                row: 4
            }
        );
        let last = source.records.last().unwrap();
        assert_eq!(
            last.location,
            SourceLocation::Row {
                sheet: "Notes".to_string(),
                row: 3
            }
        );
        assert_eq!(last.text, "Closing balance");
    }

    #[test]
    fn a_cell_holding_only_a_time_is_shown_as_a_time() {
        let mut workbook = Workbook::new();
        let sheet = workbook.add_worksheet();
        sheet
            .write_datetime_with_format(
                0,
                0,
                ExcelDateTime::from_hms(14, 14, 20).unwrap(),
                &Format::new().set_num_format("hh:mm:ss"),
            )
            .unwrap();
        sheet
            .write_datetime_with_format(
                0,
                1,
                ExcelDateTime::from_ymd(2024, 1, 2)
                    .unwrap()
                    .and_hms(9, 5, 7)
                    .unwrap(),
                &Format::new().set_num_format("dd/mm/yyyy hh:mm"),
            )
            .unwrap();
        let source = read(&workbook.save_to_buffer().unwrap()).unwrap();

        assert_eq!(
            source.records[0].fields.as_ref().unwrap(),
            &vec!["14:14:20", "2024-01-02 09:05:07"]
        );
    }
}
