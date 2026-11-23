//! The `forward!` block of a shim, read back: `strangle check` (rule 1 and "has the area moved")
//! reads the entries between the two marker lines. The macro and the tool that wrote the block
//! are gone (core-dismantle T15); only the reader stays until the `strangle` tool is deleted.

pub const FORWARD_BEGIN: &str = "// forward:begin";
pub const FORWARD_END: &str = "// forward:end";

/// Indices of the begin and end marker lines.
fn region_lines(lines: &[&str]) -> Option<(usize, usize)> {
    let marker = |wanted: &str| lines.iter().position(|line| line.trim() == wanted);
    let begin = marker(FORWARD_BEGIN)?;
    let end = marker(FORWARD_END).filter(|&end| end > begin)?;
    Some((begin, end))
}

/// One entry of a forward block: the method name and its signature as one normalized line.
#[derive(Debug, PartialEq, Eq)]
pub struct ForwardEntry {
    pub name: String,
    pub signature: String,
}

/// The entries of the forward block of a shim (none for a shim without markers). An entry is the
/// `pub fn ...` lines up to the `=>` line that names its target.
pub fn forwarded_entries(shim: &str) -> Option<Vec<ForwardEntry>> {
    let lines: Vec<&str> = shim.split_inclusive('\n').collect();
    let (begin, end) = region_lines(&lines)?;
    let mut entries = Vec::new();
    let mut open: Option<Vec<&str>> = None;
    for line in lines[begin + 1..end].iter().map(|line| line.trim()) {
        if line.starts_with("=>") {
            entries.extend(open.take().and_then(|parts| entry_from(&parts)));
        } else if let Some(parts) = open.as_mut() {
            parts.push(line);
        } else if line.starts_with("pub ") {
            open = Some(vec![line]);
        }
    }
    Some(entries)
}

fn entry_from(parts: &[&str]) -> Option<ForwardEntry> {
    let signature = parts
        .join(" ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let after_fn = signature.split_once("fn ")?.1;
    let end = after_fn.find(|ch: char| !(ch.is_alphanumeric() || ch == '_'))?;
    Some(ForwardEntry {
        name: after_fn[..end].to_string(),
        signature,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strangle::area_fixture::{SERVICE, moved_tree};

    #[test]
    fn forwarded_entries_reads_names_and_signatures_back() {
        let shim =
            std::fs::read_to_string(moved_tree(|_, text| text).root().join(SERVICE)).unwrap();
        let entries = forwarded_entries(&shim).unwrap();
        let listed: Vec<(&str, &str)> = entries
            .iter()
            .map(|entry| (entry.name.as_str(), entry.signature.as_str()))
            .collect();
        assert_eq!(
            listed,
            [
                (
                    "set_info",
                    "pub fn set_info(&mut self, id: &str, info: Info) -> Result<Store>"
                ),
                ("clash", "pub fn clash(&self, id: &str) -> bool"),
            ]
        );
        assert_eq!(forwarded_entries("pub fn x() {}"), None);
    }
}
