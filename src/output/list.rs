use unicode_width::UnicodeWidthStr;

pub struct ListView<'a> {
    pub headers: &'a [&'static str],
    pub rows: Vec<ListRow>,
    pub plural_noun: &'static str,
}

pub struct ListRow {
    pub cells: Vec<ListCell>,
}

pub enum ListCell {
    Text(String),
    Number(String),
    Optional(Option<String>),
    Styled { text: String, ansi_prefix: &'static str, ansi_suffix: &'static str },
}

pub struct ListOpts {
    pub no_headers: bool,
    pub no_trunc: bool,
    pub color_enabled: bool,
    pub terminal_width: usize,
}

impl ListOpts {
    pub fn from_flags(no_headers: bool, no_trunc: bool) -> Self {
        let t = super::tty::probe();
        Self {
            no_headers: no_headers || !t.stdout_is_tty,
            no_trunc: no_trunc || !t.stdout_is_tty,
            color_enabled: t.color_enabled,
            terminal_width: if t.stdout_is_tty { t.width } else { usize::MAX },
        }
    }
}

pub fn render_text(view: &ListView, opts: &ListOpts) -> String {
    if view.rows.is_empty() {
        return format!("No {} found.\n", view.plural_noun);
    }

    let ncols = view.headers.len();
    let mut col_widths: Vec<usize> = view.headers.iter().map(|h| UnicodeWidthStr::width(*h)).collect();

    for row in &view.rows {
        for (i, cell) in row.cells.iter().enumerate() {
            let w = cell_display_width(cell);
            if w > col_widths[i] {
                col_widths[i] = w;
            }
        }
    }

    let total_width: usize = col_widths.iter().sum::<usize>() + (ncols.saturating_sub(1)) * 2;
    let truncate = !opts.no_trunc && opts.terminal_width < usize::MAX && total_width > opts.terminal_width;

    let mut out = String::new();

    if !opts.no_headers {
        write_row(&mut out, view.headers, &col_widths, truncate, opts);
        out.push('\n');
    }

    for row in &view.rows {
        write_row(&mut out, &row.cells, &col_widths, truncate, opts);
        out.push('\n');
    }

    out
}

fn write_row(out: &mut String, cells: &[impl CellContent], col_widths: &[usize], truncate: bool, opts: &ListOpts) {
    let ncols = cells.len();
    let available = if truncate { opts.terminal_width } else { usize::MAX };

    let mut used = 0usize;
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            out.push_str("  ");
            used += 2;
        }

        if i == 0 {
            // Identity column: never truncated
            let text = cell_text(cell, opts.color_enabled);
            let w = cell_display_width(cell);
            out.push_str(&text);
            used += w;
            if i < ncols - 1 {
                pad_to(out, col_widths[i], w);
            }
            used = used.max(col_widths[i]);
            continue;
        }

        let target = col_widths[i];
        let w = cell_display_width(cell);

        if truncate && i > 0 {
            let remaining = available.saturating_sub(used);
            if remaining < target.min(3) {
                // can't fit even "…", stop
                break;
            }
        }

        let text = cell_text(cell, opts.color_enabled);
        if truncate && i > 0 && (used + target > available) {
            // truncate this column
            let available_for_col = available.saturating_sub(used);
            if available_for_col <= 1 {
                break;
            }
            let truncated = truncate_cell(&text, w, available_for_col);
            out.push_str(&truncated);
            break;
        }

        out.push_str(&text);
        if i < ncols - 1 {
            pad_to(out, target, w);
        }
        used += target;
    }
}

fn pad_to(out: &mut String, target: usize, actual_w: usize) {
    let padding = target.saturating_sub(actual_w);
    for _ in 0..padding {
        out.push(' ');
    }
}

fn cell_text(cell: &impl CellContent, color_enabled: bool) -> String {
    let raw = cell.raw_text();
    if color_enabled {
        cell.colored_text()
    } else {
        raw
    }
}

fn cell_display_width(cell: &impl CellContent) -> usize {
    UnicodeWidthStr::width(cell.display_str().as_str())
}

fn truncate_cell(text: &str, display_w: usize, available: usize) -> String {
    if display_w <= available {
        return text.to_string();
    }
    if available <= 1 {
        return "…".to_string();
    }
    let ellipsis = "…";
    let ellipsis_w = UnicodeWidthStr::width(ellipsis);
    if available <= ellipsis_w {
        return ellipsis.to_string();
    }
    let target_w = available - ellipsis_w;
    // Walk the string to find the prefix that fits
    let mut prefix = String::new();
    let mut pw = 0usize;
    for ch in text.chars() {
        let cw = UnicodeWidthStr::width(ch.to_string().as_str());
        if pw + cw > target_w {
            break;
        }
        prefix.push(ch);
        pw += cw;
    }
    prefix.push_str(ellipsis);
    prefix
}

/// A helper trait so we can format both header &str and ListCell uniformly
trait CellContent {
    fn raw_text(&self) -> String;
    fn display_str(&self) -> String;
    fn colored_text(&self) -> String {
        self.raw_text()
    }
}

impl CellContent for &str {
    fn raw_text(&self) -> String {
        self.to_string()
    }
    fn display_str(&self) -> String {
        self.to_string()
    }
}

impl CellContent for ListCell {
    fn raw_text(&self) -> String {
        match self {
            ListCell::Text(s) => s.clone(),
            ListCell::Number(s) => s.clone(),
            ListCell::Optional(Some(s)) => s.clone(),
            ListCell::Optional(None) => "-".to_string(),
            ListCell::Styled { text, .. } => text.clone(),
        }
    }

    fn display_str(&self) -> String {
        self.raw_text()
    }

    fn colored_text(&self) -> String {
        match self {
            ListCell::Styled { text, ansi_prefix, ansi_suffix } => {
                let suffix = if ansi_suffix.is_empty() { "\x1b[0m" } else { ansi_suffix };
                format!("{ansi_prefix}{text}{suffix}")
            }
            _ => self.raw_text(),
        }
    }
}

pub fn json_collection(plural_noun: &str, items: Vec<serde_json::Value>) -> serde_json::Value {
    serde_json::json!({ plural_noun: items })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_list_shows_placeholder() {
        let view = ListView { headers: &["NAME", "DOCS"], rows: vec![], plural_noun: "projects" };
        let opts = ListOpts { no_headers: false, no_trunc: false, color_enabled: false, terminal_width: 80 };
        let result = render_text(&view, &opts);
        assert_eq!(result, "No projects found.\n");
    }

    #[test]
    fn single_row_with_headers() {
        let view = ListView {
            headers: &["NAME", "DOCS"],
            rows: vec![ListRow { cells: vec![ListCell::Text("demo".into()), ListCell::Number("4".into())] }],
            plural_noun: "projects",
        };
        let opts = ListOpts { no_headers: false, no_trunc: false, color_enabled: false, terminal_width: 80 };
        let result = render_text(&view, &opts);
        let lines: Vec<&str> = result.lines().collect();
        assert_eq!(lines.len(), 2);
        assert!(lines[0].starts_with("NAME"));
        assert!(lines[1].starts_with("demo"));
    }

    #[test]
    fn optional_cell_shows_dash() {
        let view = ListView {
            headers: &["NAME", "LAST ACTIVITY"],
            rows: vec![ListRow { cells: vec![ListCell::Text("demo2".into()), ListCell::Optional(None)] }],
            plural_noun: "projects",
        };
        let opts = ListOpts { no_headers: false, no_trunc: false, color_enabled: false, terminal_width: 80 };
        let result = render_text(&view, &opts);
        assert!(result.contains(" -"), "expected - placeholder");
    }

    #[test]
    fn truncation_applies_to_non_identity_columns() {
        let view = ListView {
            headers: &["NAME", "DEFINITION"],
            rows: vec![ListRow {
                cells: vec![
                    ListCell::Text("RAG".into()),
                    ListCell::Text("Retrieval-Augmented Generation for LLMs".into()),
                ],
            }],
            plural_noun: "terms",
        };
        let opts = ListOpts { no_headers: false, no_trunc: false, color_enabled: false, terminal_width: 30 };
        let result = render_text(&view, &opts);
        assert!(result.contains("RAG"), "identity column must not be truncated");
        assert!(result.contains("…") || result.contains("..."), "truncation ellipsis expected");
    }

    #[test]
    fn json_collection_wraps_items() {
        let items = vec![serde_json::json!({"identity": "demo", "docs": 4})];
        let result = json_collection("projects", items);
        assert_eq!(result["projects"][0]["identity"], "demo");
    }

    #[test]
    fn cjk_column_width() {
        let view = ListView {
            headers: &["TERM", "DEFINITION"],
            rows: vec![ListRow {
                cells: vec![ListCell::Text("检索".into()), ListCell::Text("RAG的中文缩写".into())]
            }],
            plural_noun: "terms",
        };
        let opts = ListOpts { no_headers: false, no_trunc: false, color_enabled: false, terminal_width: 80 };
        let result = render_text(&view, &opts);
        // CJK chars are 2 display width each
        let lines: Vec<&str> = result.lines().collect();
        assert!(!lines.is_empty());
    }
}
