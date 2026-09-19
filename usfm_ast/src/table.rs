use super::{Inline, Span};

#[derive(Debug, PartialEq)]
pub struct Table<'a> {
    pub rows: Vec<TableRow<'a>>,
    /// Source range from the first `\tr` to the end of the last cell.
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct TableRow<'a> {
    pub cells: Vec<TableCell<'a>>,
    /// Source range of the `\tr` marker and the row's cells.
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub struct TableCell<'a> {
    pub header: bool,
    pub alignment: Alignment,
    /// The column the marker names, 1-based: `\tc3` is 3, `\tc2-3` is 2.
    /// Kept from the source rather than counted, so a skipped column
    /// (`\th1 … \th3`) round-trips and is reported, not silently renumbered.
    pub column: u8,
    pub colspan: u8,
    pub children: Vec<Inline<'a>>,
    /// Source range of the cell marker and its content.
    pub span: Span,
}

#[derive(Debug, PartialEq)]
pub enum Alignment {
    Start,
    Center,
    End,
}

impl std::fmt::Display for Alignment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Alignment::Start => "start",
                Alignment::Center => "center",
                Alignment::End => "end",
            }
        )
    }
}
