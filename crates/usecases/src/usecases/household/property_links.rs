//! Scenarios of the property-links area: a link is checked against the live
//! stores (an area that has not moved yet) and set inside the caller's own
//! transaction, so nothing here writes to disk by itself.

use crate::error::Result;
use crate::household::property_links::{replace_link, require_property};
use crate::service::Session;

/// A link may point only at a live property.
pub fn check_property_link(vault: &Session, property_id: Option<&str>) -> Result<()> {
    let Some(id) = property_id else {
        return Ok(());
    };
    require_property(crate::money::validate::live_store(&vault.doc, id)?)
}

/// Link the operation to a property, or unlink it with `None`. Not persisted:
/// the operation that carries the link is.
pub fn set_property_link(vault: &mut Session, tx_id: &str, property_id: Option<&str>) {
    replace_link(&mut vault.doc, tx_id, property_id);
}
