//! HTML pages: a safe copy for the picker and the values a method points at.

use scraper::{ElementRef, Html, Node, Selector};

use crate::model::datasource::TableRow;

/// Elements dropped with everything inside them: they run code, submit,
/// embed other pages or redirect.
const DROPPED: &[&str] = &[
    "script", "noscript", "iframe", "frame", "frameset", "object", "embed", "form", "base",
    "applet", "portal",
];
const VOID: &[&str] = &[
    "area", "br", "col", "hr", "img", "input", "link", "meta", "source", "track", "wbr",
];
/// Raw-text elements whose content is not escaped.
const RAW_TEXT: &[&str] = &["style"];

/// The picker's policy: nothing runs, nothing connects, nothing submits;
/// pictures and styles still load so the page looks like itself.
pub const PREVIEW_CSP: &str = "default-src 'none'; img-src * data:; style-src * 'unsafe-inline'; font-src * data:; \
     script-src 'none'; connect-src 'none'; form-action 'none'; frame-src 'none'";

/// The page without scripts, event handlers, `javascript:` links, forms and
/// frames, with the CSP above and a `<base>` so relative pictures load.
pub fn sanitize(html: &str, base_url: &str) -> String {
    let doc = Html::parse_document(html);
    let mut out = String::with_capacity(html.len());
    out.push_str("<!doctype html>");
    write_children(doc.tree.root(), &mut out, base_url);
    out
}

fn write_children(node: ego_tree::NodeRef<'_, Node>, out: &mut String, base_url: &str) {
    for child in node.children() {
        write_node(child, out, base_url);
    }
}

fn write_node(node: ego_tree::NodeRef<'_, Node>, out: &mut String, base_url: &str) {
    match node.value() {
        Node::Text(text) => {
            let raw = node
                .parent()
                .and_then(|parent| parent.value().as_element())
                .is_some_and(|parent| RAW_TEXT.contains(&parent.name()));
            if raw {
                out.push_str(&text.replace("</", "<\\/"));
            } else {
                out.push_str(&escape(text));
            }
        }
        Node::Element(element) => {
            let name = element.name();
            if DROPPED.contains(&name) {
                return;
            }
            let refresh = name == "meta"
                && element
                    .attr("http-equiv")
                    .is_some_and(|value| !value.eq_ignore_ascii_case("content-type"));
            if refresh {
                return;
            }
            out.push('<');
            out.push_str(name);
            for (attr, value) in element.attrs() {
                if !safe_attr(attr, value) {
                    continue;
                }
                out.push(' ');
                out.push_str(attr);
                out.push_str("=\"");
                out.push_str(&escape(value));
                out.push('"');
            }
            out.push('>');
            if name == "head" {
                out.push_str(&format!(
                    "<meta http-equiv=\"Content-Security-Policy\" content=\"{PREVIEW_CSP}\">\
                     <base href=\"{}\">",
                    escape(base_url)
                ));
            }
            if VOID.contains(&name) {
                return;
            }
            write_children(node, out, base_url);
            out.push_str("</");
            out.push_str(name);
            out.push('>');
        }
        Node::Document | Node::Fragment => write_children(node, out, base_url),
        _ => {}
    }
}

fn safe_attr(attr: &str, value: &str) -> bool {
    let attr = attr.to_ascii_lowercase();
    if attr.starts_with("on") || attr == "srcdoc" || attr == "formaction" {
        return false;
    }
    let compact: String = value
        .chars()
        .filter(|ch| !ch.is_whitespace() && !ch.is_control())
        .collect::<String>()
        .to_ascii_lowercase();
    !(compact.starts_with("javascript:") || compact.starts_with("vbscript:"))
}

fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn selector(css: &str) -> Result<Selector, String> {
    Selector::parse(css).map_err(|err| format!("bad selector «{css}»: {err}"))
}

/// Visible text of an element, with runs of whitespace collapsed.
fn text_of(element: ElementRef<'_>) -> String {
    element
        .text()
        .collect::<Vec<_>>()
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The text of the first element matching `css`.
pub fn select_text(doc: &Html, css: &str) -> Result<Option<String>, String> {
    let selector = selector(css)?;
    Ok(doc.select(&selector).next().map(text_of))
}

/// The whole page text, for the label search and the `text_hint` fallback.
pub fn page_text(doc: &Html) -> String {
    text_of(doc.root_element())
}

/// The text right after `label` (case-insensitive), up to 80 characters.
pub fn text_after(page: &str, label: &str) -> Option<String> {
    let label = label.trim();
    if label.is_empty() {
        return None;
    }
    let lower = page.to_lowercase();
    let at = lower.find(&label.to_lowercase())?;
    // Lowercasing keeps byte offsets for the scripts these pages use; check
    // anyway so an odd letter cannot split a character.
    let rest = page.get(at + label.len()..)?;
    Some(rest.chars().take(80).collect())
}

/// The cells of the first table matching `css`: header row and body rows.
pub fn table_rows(doc: &Html, css: &str) -> Result<Vec<Vec<String>>, String> {
    let table_selector = selector(css)?;
    let row_selector = selector("tr")?;
    let cell_selector = selector("th, td")?;
    let table = doc
        .select(&table_selector)
        .next()
        .ok_or_else(|| format!("no table matches «{css}»"))?;
    Ok(table
        .select(&row_selector)
        .map(|row| row.select(&cell_selector).map(text_of).collect())
        .filter(|cells: &Vec<String>| !cells.is_empty())
        .collect())
}

/// Which body row a [`TableRow`] means. `by_date` reads the first cell.
pub fn pick_row(rows: &[Vec<String>], row: TableRow) -> Option<&Vec<String>> {
    match row {
        TableRow::First => rows.first(),
        TableRow::Last => rows.last(),
        TableRow::ByDate => rows
            .iter()
            .filter_map(|cells| Some((super::parse_date_label(cells.first()?)?, cells)))
            .max_by(|left, right| left.0.cmp(&right.0))
            .map(|(_, cells)| cells)
            .or_else(|| rows.first()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitize_drops_what_could_run() {
        let page = r#"<html><head><script>alert(1)</script><meta http-equiv="refresh" content="0;url=x"></head>
            <body onload="evil()"><a href="javascript:alert(1)">x</a><a href="/ok">ok</a>
            <form action="/post"><input name="q"></form><iframe src="https://x"></iframe>
            <img src="a.png" onerror="evil()"><p style="color:red">6,35 %</p></body></html>"#;
        let safe = sanitize(page, "https://cbr.ru/");
        assert!(!safe.contains("<script"));
        assert!(!safe.contains("alert"));
        assert!(!safe.contains("onload"));
        assert!(!safe.contains("onerror"));
        assert!(!safe.contains("<form"));
        assert!(!safe.contains("<iframe"));
        assert!(!safe.contains("refresh"));
        assert!(safe.contains("Content-Security-Policy"));
        assert!(safe.contains("<base href=\"https://cbr.ru/\">"));
        assert!(safe.contains("href=\"/ok\""));
        assert!(safe.contains("6,35 %"));
    }

    #[test]
    fn reads_a_table_by_rows() {
        let doc = Html::parse_document(
            "<table class=t><tr><th>Дата</th><th>Инфляция</th></tr>\
             <tr><td>08.2026</td><td>6,40</td></tr><tr><td>09.2026</td><td>6,35</td></tr></table>",
        );
        let rows = table_rows(&doc, "table.t").unwrap();
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0], vec!["Дата", "Инфляция"]);
        let body = &rows[1..];
        assert_eq!(pick_row(body, TableRow::ByDate).unwrap()[1], "6,35");
        assert_eq!(pick_row(body, TableRow::First).unwrap()[1], "6,40");
    }

    #[test]
    fn text_after_a_label() {
        assert_eq!(
            text_after("Ключевая ставка 16,50 % с 01.09", "ключевая ставка").as_deref(),
            Some(" 16,50 % с 01.09")
        );
        assert_eq!(text_after("abc", "zzz"), None);
    }
}
