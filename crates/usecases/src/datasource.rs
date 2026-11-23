//! Own data sources (page elements, JSON, CSV, XLSX): the facade of `mwt-net`.

pub use mwt_model::Extracted;
pub use mwt_net::{PreviewKind, SourcePreview, extract, parse_date_label, parse_number, preview};
