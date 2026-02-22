use serde::{Deserialize, Serialize};
use crate::ocr::OcrWord;
use crate::scanner::ScannerError;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectedTable {
    pub rows: Vec<Vec<String>>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub row_count: usize,
    pub col_count: usize,
}

/// Detect tables from OCR word bounding boxes by analyzing alignment patterns.
/// Words that share similar Y positions form rows, and X alignment forms columns.
pub fn detect_tables(words: &[OcrWord], img_width: u32, img_height: u32) -> Vec<DetectedTable> {
    if words.is_empty() {
        return Vec::new();
    }

    // Group words by approximate Y position (row detection)
    let y_tolerance = 10; // pixels
    let mut rows: Vec<Vec<&OcrWord>> = Vec::new();

    let mut sorted_words: Vec<&OcrWord> = words.iter().collect();
    sorted_words.sort_by_key(|w| w.y);

    let mut current_row: Vec<&OcrWord> = Vec::new();
    let mut current_y = sorted_words[0].y;

    for word in &sorted_words {
        if (word.y - current_y).abs() <= y_tolerance {
            current_row.push(word);
        } else {
            if !current_row.is_empty() {
                rows.push(current_row);
            }
            current_row = vec![word];
            current_y = word.y;
        }
    }
    if !current_row.is_empty() {
        rows.push(current_row);
    }

    // Sort words within each row by X position
    for row in &mut rows {
        row.sort_by_key(|w| w.x);
    }

    // Detect table-like regions: consecutive rows with similar column counts
    let mut tables = Vec::new();
    let mut table_start = 0;

    while table_start < rows.len() {
        let col_count = rows[table_start].len();

        // Only consider rows with 2+ columns as potential table rows
        if col_count < 2 {
            table_start += 1;
            continue;
        }

        let mut table_end = table_start + 1;

        // Find consecutive rows with similar column count (within +/- 1)
        while table_end < rows.len() {
            let next_cols = rows[table_end].len();
            if (next_cols as i32 - col_count as i32).unsigned_abs() <= 1 && next_cols >= 2 {
                table_end += 1;
            } else {
                break;
            }
        }

        // Need at least 2 rows for a table
        if table_end - table_start >= 2 {
            let table_rows = &rows[table_start..table_end];

            // Determine column boundaries from X positions
            let max_cols = table_rows.iter().map(|r| r.len()).max().unwrap_or(0);

            let mut parsed_rows = Vec::new();
            for row in table_rows {
                let mut cells: Vec<String> = Vec::new();
                for word in row.iter() {
                    cells.push(word.text.clone());
                }
                // Pad to max_cols
                while cells.len() < max_cols {
                    cells.push(String::new());
                }
                parsed_rows.push(cells);
            }

            // Calculate bounding box
            let all_words: Vec<&&OcrWord> = table_rows.iter().flat_map(|r| r.iter()).collect();
            let min_x = all_words.iter().map(|w| w.x).min().unwrap_or(0) as f64;
            let min_y = all_words.iter().map(|w| w.y).min().unwrap_or(0) as f64;
            let max_x = all_words.iter().map(|w| w.x + w.w).max().unwrap_or(0) as f64;
            let max_y = all_words.iter().map(|w| w.y + w.h).max().unwrap_or(0) as f64;

            let row_count = parsed_rows.len();

            tables.push(DetectedTable {
                rows: parsed_rows,
                x: min_x / img_width as f64,
                y: min_y / img_height as f64,
                width: (max_x - min_x) / img_width as f64,
                height: (max_y - min_y) / img_height as f64,
                row_count,
                col_count: max_cols,
            });
        }

        table_start = table_end;
    }

    tables
}

/// Export a detected table as CSV string.
pub fn table_to_csv(table: &DetectedTable) -> String {
    let mut csv = String::new();
    for row in &table.rows {
        let line: Vec<String> = row
            .iter()
            .map(|cell| {
                if cell.contains(',') || cell.contains('"') || cell.contains('\n') {
                    format!("\"{}\"", cell.replace('"', "\"\""))
                } else {
                    cell.clone()
                }
            })
            .collect();
        csv.push_str(&line.join(","));
        csv.push('\n');
    }
    csv
}
