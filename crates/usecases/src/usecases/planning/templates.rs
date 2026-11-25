//! Scenarios of the operation templates (spec op-period T3): each decides in `planning::templates`,
//! then ends in one `vault.transact`. Delete returns the template and restore takes it back, as
//! the other generic deletes do.

use crate::error::Result;
use crate::model::vault::now_iso;
use crate::planning::automation_plan as plan;
use crate::planning::templates::{
    TemplateDraft, plan_archive_template, plan_create_template, plan_delete_template,
    plan_restore_template, plan_update_template,
};
use crate::service::Session;
use mwt_model::OperationTemplate;

/// Every template, live and archived, by name.
pub fn list_operation_templates(vault: &Session) -> Vec<OperationTemplate> {
    let mut templates = vault.doc.operation_templates.clone();
    templates.sort_by_key(|template| template.name.to_lowercase());
    templates
}

pub fn create_operation_template(
    vault: &mut Session,
    draft: TemplateDraft,
) -> Result<OperationTemplate> {
    let template = plan_create_template(
        &vault.doc,
        draft,
        uuid::Uuid::new_v4().to_string(),
        &now_iso(),
    )?;
    vault.transact(|tx| {
        tx.collection::<OperationTemplate>()
            .insert(template.clone())?;
        Ok(template)
    })
}

pub fn update_operation_template(
    vault: &mut Session,
    id: &str,
    draft: TemplateDraft,
) -> Result<OperationTemplate> {
    let template = plan_update_template(&vault.doc, id, draft)?;
    replace(vault, template)
}

/// Archive (`archived = true`) or bring a template back; allowed while automations use it.
pub fn archive_operation_template(
    vault: &mut Session,
    id: &str,
    archived: bool,
) -> Result<OperationTemplate> {
    let template = plan_archive_template(&vault.doc, id, archived)?;
    replace(vault, template)
}

/// Delete a template, refused while an automation refers to it. The result undoes it.
pub fn delete_operation_template(vault: &mut Session, id: &str) -> Result<OperationTemplate> {
    let automations = plan::automations(&vault.books());
    let deleted = plan_delete_template(&vault.doc, id, &automations)?;
    vault.transact(|tx| {
        tx.collection::<OperationTemplate>().remove(id)?;
        Ok(())
    })?;
    Ok(deleted)
}

/// Undo [`delete_operation_template`].
pub fn restore_operation_template(
    vault: &mut Session,
    deleted: OperationTemplate,
) -> Result<OperationTemplate> {
    plan_restore_template(&vault.doc, &deleted)?;
    vault.transact(|tx| {
        tx.collection::<OperationTemplate>()
            .insert(deleted.clone())?;
        Ok(deleted)
    })
}

fn replace(vault: &mut Session, template: OperationTemplate) -> Result<OperationTemplate> {
    vault.transact(|tx| {
        tx.collection::<OperationTemplate>()
            .update(&template.id.clone(), |slot| *slot = template.clone())?;
        Ok(template)
    })
}
