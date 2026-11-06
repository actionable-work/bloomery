use bloomery_model::SourceLocation;
use std::ops::Range;
use std::path::Path;

/// Maps parser byte offsets to one-based Unicode scalar positions.
pub(crate) struct SourceMap<'a> {
    source: &'a str,
    lines: Vec<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg_attr(any(), bloomery("PARSER-SCANNING-SPANS-003"))]
    fn maps_unicode_tabs_crlf_and_empty_ranges_without_byte_column_confusion() {
        let source = "é🦀\tX\r\nz";
        let map = SourceMap::new(source);
        let location = map.location(Path::new("source"), 7..11);
        assert_eq!((location.line, location.column), (Some(1), Some(4)));
        assert_eq!((location.end_line, location.end_column), (Some(2), Some(2)));
        let empty = SourceMap::new("").location(Path::new("empty"), 0..0);
        assert_eq!(
            (empty.line, empty.column, empty.end_line, empty.end_column),
            (Some(1), Some(1), Some(1), Some(1))
        );
        for range in [1..2, 0..12] {
            let unknown = map.location(Path::new("bad"), range);
            assert_eq!(
                (
                    unknown.line,
                    unknown.column,
                    unknown.end_line,
                    unknown.end_column
                ),
                (None, None, None, None)
            );
        }
    }
}

impl<'a> SourceMap<'a> {
    pub(crate) fn new(source: &'a str) -> Self {
        Self {
            source,
            lines: std::iter::once(0)
                .chain(source.match_indices('\n').map(|(index, _)| index + 1))
                .collect(),
        }
    }

    pub(crate) fn location(&self, path: &Path, range: Range<usize>) -> SourceLocation {
        let mut location = SourceLocation::new(path, None);
        if self.source.get(range.clone()).is_some() {
            let (line, column) = self.position(range.start);
            let (end_line, end_column) = self.position(range.end);
            location.line = Some(line);
            location.column = Some(column);
            location.end_line = Some(end_line);
            location.end_column = Some(end_column);
        }
        location
    }

    fn position(&self, offset: usize) -> (usize, usize) {
        let line = self.lines.partition_point(|start| *start <= offset);
        (
            line,
            self.source[self.lines[line - 1]..offset].chars().count() + 1,
        )
    }
}
