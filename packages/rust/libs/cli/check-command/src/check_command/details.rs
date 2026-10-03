use super::model::DISPLAY_RECORD_BYTE_LIMIT;

#[derive(Clone, Copy)]
enum EscapeState {
    Normal,
    Escape,
    Csi,
    Osc,
    OscEscape,
}

pub fn normalize_display_bytes(bytes: &[u8]) -> Vec<String> {
    let text = String::from_utf8_lossy(bytes);
    display_records(&normalize_display_text(&text))
}

pub fn normalize_display_text(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut state = EscapeState::Normal;
    let mut after_carriage_return = false;

    for character in text.chars() {
        match state {
            EscapeState::Normal => {
                if after_carriage_return {
                    after_carriage_return = false;
                    if character == '\n' {
                        continue;
                    }
                }
                match character {
                    '\u{001b}' => state = EscapeState::Escape,
                    '\u{009b}' => state = EscapeState::Csi,
                    '\u{009d}' => state = EscapeState::Osc,
                    '\r' => {
                        output.push('\n');
                        after_carriage_return = true;
                    }
                    '\n' => output.push('\n'),
                    '\t' => output.push(' '),
                    value if value.is_control() => {}
                    value => output.push(value),
                }
            }
            EscapeState::Escape => match character {
                '[' => state = EscapeState::Csi,
                ']' => state = EscapeState::Osc,
                '\u{001b}' => state = EscapeState::Escape,
                _ => state = EscapeState::Normal,
            },
            EscapeState::Csi => {
                if ('@'..='~').contains(&character) {
                    state = EscapeState::Normal;
                } else if character == '\u{001b}' {
                    state = EscapeState::Escape;
                }
            }
            EscapeState::Osc => match character {
                '\u{0007}' | '\u{009c}' => state = EscapeState::Normal,
                '\u{001b}' => state = EscapeState::OscEscape,
                _ => {}
            },
            EscapeState::OscEscape => match character {
                '\\' | '\u{009c}' => state = EscapeState::Normal,
                '\u{001b}' => state = EscapeState::OscEscape,
                _ => state = EscapeState::Osc,
            },
        }
    }
    output
}

pub fn display_records(text: &str) -> Vec<String> {
    if text.is_empty() {
        return Vec::new();
    }
    let logical_lines = text.strip_suffix('\n').unwrap_or(text);
    let mut records = Vec::new();
    for line in logical_lines.split('\n') {
        if line.is_empty() {
            records.push(String::new());
            continue;
        }
        let mut current = String::new();
        for character in line.chars() {
            if current.len() + character.len_utf8() > DISPLAY_RECORD_BYTE_LIMIT {
                records.push(std::mem::take(&mut current));
            }
            current.push(character);
        }
        if !current.is_empty() {
            records.push(current);
        }
    }
    records
}

pub fn diagnostic_detail_text(
    code: &str,
    message: &str,
    location: Option<&super::model::FailureLocation>,
    notes: &[String],
) -> String {
    let mut text = format!("ERROR [{code}]: {message}\n");
    if let Some(location) = location {
        text.push_str(&format!("  --> {}", location.path));
        if let Some(line) = location.line {
            text.push_str(&format!(":{line}"));
            if let Some(column) = location.column {
                text.push_str(&format!(":{column}"));
            }
        }
        text.push('\n');
    }
    for note in notes {
        text.push_str(&format!("  = note: {note}\n"));
    }
    normalize_display_text(&text)
}

#[cfg(test)]
mod tests {
    use super::{display_records, normalize_display_bytes, normalize_display_text};
    use bloomery_test_macros::bloomery;

    #[test]
    #[bloomery("CLI-CHECK-DETAIL-013")]
    #[bloomery("CLI-CHECK-DETAIL-020")]
    fn oversized_logical_lines_split_into_seekable_bounded_records() {
        let input = "é".repeat(600);
        let records = display_records(&input);
        assert_eq!(records.len(), 3);
        assert!(records.iter().all(|record| record.len() <= 512));
        assert_eq!(records.concat(), input);
    }

    #[test]
    #[bloomery("CLI-CHECK-DETAIL-015")]
    fn display_text_replaces_invalid_utf8_and_removes_terminal_controls() {
        let records = normalize_display_bytes(
            b"first\r\n\x1b[31mred\x1b[0m\x1b]0;private-title\x07\x01bad\xff\n",
        );
        assert_eq!(records, vec!["first", "redbad�"]);
        let normalized = normalize_display_text("a\tb\x1b[2Jc\u{009b}31md");
        assert_eq!(normalized, "a bcd");
        assert!(!normalized.chars().any(char::is_control));
    }
}
