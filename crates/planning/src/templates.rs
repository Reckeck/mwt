//! Operation templates (spec op-period T2, T3, D6, D7): the rules a template must keep when it is
//! saved, what deleting one takes, and the repair of references that went missing after a merge.
//! Pure; the usecase reads the vault, calls these and writes.

use mwt_base::error::{Error, Result};
use mwt_model::{
    Automation, DistKind, DistributionPart, DistributionRule, OperationTemplate, PartStatus,
    PeriodRule, TemplateKind, TxKind, VaultDoc,
};
use mwt_money::distribution::{REMAINDER_COUNT, validate_distribution_targets};

/// What the user saves; the id, the archive mark and the creation time are not theirs to set.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct TemplateDraft {
    pub name: String,
    pub kind: TemplateKind,
    #[serde(default)]
    pub store_id: Option<String>,
    #[serde(default)]
    pub asset: Option<mwt_model::AssetRef>,
    #[serde(default)]
    pub amount: Option<mwt_base::Amount>,
    #[serde(default)]
    pub categories: Vec<String>,
    #[serde(default)]
    pub comment: String,
    #[serde(default)]
    pub contact_id: Option<String>,
    #[serde(default)]
    pub period_rule: Option<PeriodRule>,
    #[serde(default)]
    pub distribution: Option<DistributionRule>,
}

fn same_name(left: &str, right: &str) -> bool {
    left.trim().to_lowercase() == right.trim().to_lowercase()
}

/// T2: the rules of one template against the document. Its own record (an edit) is not a
/// duplicate of itself.
pub fn check_template(doc: &VaultDoc, template: &OperationTemplate) -> Result<()> {
    if template.name.trim().is_empty() {
        return Err(Error::InvalidInput("name the template".into()));
    }
    let taken = !template.archived
        && doc.operation_templates.iter().any(|known| {
            known.id != template.id && !known.archived && same_name(&known.name, &template.name)
        });
    if taken {
        return Err(Error::InvalidInput(format!(
            "a template named {} already exists",
            template.name.trim()
        )));
    }
    check_references(doc, template)?;
    match &template.distribution {
        Some(_) if template.kind != TemplateKind::Income => Err(Error::InvalidInput(
            "only an income template can carry a distribution".into(),
        )),
        Some(rule) => check_distribution_rule(doc, rule),
        None => Ok(()),
    }
}

fn check_references(doc: &VaultDoc, template: &OperationTemplate) -> Result<()> {
    for key in &template.categories {
        if !doc.categories.iter().any(|category| &category.key == key) {
            return Err(Error::Missing(format!("category {key}")));
        }
    }
    if let Some(id) = &template.store_id
        && !doc.stores.iter().any(|store| &store.id == id)
    {
        return Err(Error::Missing(format!("store {id}")));
    }
    if let Some(id) = &template.contact_id
        && !doc.contacts.iter().any(|contact| &contact.id == id)
    {
        return Err(Error::Missing(format!("contact {id}")));
    }
    Ok(())
}

/// D7: ordered rows, exactly one remainder, targets that exist and are own (S6, S7).
fn check_distribution_rule(doc: &VaultDoc, rule: &DistributionRule) -> Result<()> {
    let remainders = rule
        .rows
        .iter()
        .filter(|row| matches!(row.kind, DistKind::Remainder))
        .count();
    if remainders != 1 {
        return Err(Error::InvalidInput(format!(
            "{REMAINDER_COUNT}: a distribution needs exactly one remainder, got {remainders}"
        )));
    }
    let parts: Vec<DistributionPart> = rule
        .rows
        .iter()
        .map(|row| DistributionPart {
            kind: row.kind.clone(),
            target: row.target.clone(),
            amount: mwt_base::Amount::ZERO,
            status: PartStatus::Done,
            effect: None,
        })
        .collect();
    validate_distribution_targets(doc, TxKind::Topup, &parts)
}

/// A new template from a draft, checked.
pub fn plan_create_template(
    doc: &VaultDoc,
    draft: TemplateDraft,
    fresh_id: String,
    now: &str,
) -> Result<OperationTemplate> {
    let template = from_draft(draft, fresh_id, false, now.to_string());
    check_template(doc, &template)?;
    Ok(template)
}

/// The template `id` with the draft's content, checked; id, archive mark and creation time stay.
pub fn plan_update_template(
    doc: &VaultDoc,
    id: &str,
    draft: TemplateDraft,
) -> Result<OperationTemplate> {
    let old = find_template(doc, id)?;
    let template = from_draft(draft, old.id.clone(), old.archived, old.created_at.clone());
    check_template(doc, &template)?;
    Ok(template)
}

/// The template `id` with its archive mark set (archiving is allowed while automations use it).
/// Unarchiving checks the name against the live ones again.
pub fn plan_archive_template(
    doc: &VaultDoc,
    id: &str,
    archived: bool,
) -> Result<OperationTemplate> {
    let mut template = find_template(doc, id)?.clone();
    template.archived = archived;
    if !archived {
        check_template(doc, &template)?;
    }
    Ok(template)
}

/// T3: the template to delete, unless an automation still refers to it.
pub fn plan_delete_template(
    doc: &VaultDoc,
    id: &str,
    automations: &[Automation],
) -> Result<OperationTemplate> {
    let template = find_template(doc, id)?.clone();
    if let Some(user) = automations
        .iter()
        .find(|automation| automation.operation_template_id.as_deref() == Some(id))
    {
        return Err(Error::InvalidInput(format!(
            "template {} is used by the automation {}; archive it instead",
            template.name, user.name
        )));
    }
    Ok(template)
}

/// Undo of a delete: the template returns as it was unless its id or live name is taken now.
pub fn plan_restore_template(doc: &VaultDoc, deleted: &OperationTemplate) -> Result<()> {
    if doc
        .operation_templates
        .iter()
        .any(|known| known.id == deleted.id)
    {
        return Err(Error::InvalidInput(format!(
            "template {} already exists",
            deleted.id
        )));
    }
    check_template(doc, deleted)
}

fn find_template<'doc>(doc: &'doc VaultDoc, id: &str) -> Result<&'doc OperationTemplate> {
    doc.operation_templates
        .iter()
        .find(|template| template.id == id)
        .ok_or_else(|| Error::Missing(format!("template {id}")))
}

fn from_draft(
    draft: TemplateDraft,
    id: String,
    archived: bool,
    created_at: String,
) -> OperationTemplate {
    OperationTemplate {
        id,
        name: draft.name.trim().to_string(),
        archived,
        created_at,
        kind: draft.kind,
        store_id: draft.store_id,
        asset: draft.asset,
        amount: draft.amount,
        categories: draft.categories,
        comment: draft.comment,
        contact_id: draft.contact_id,
        period_rule: draft.period_rule,
        distribution: draft.distribution,
    }
}

/// Repair (T2): drop the categories, the store and the contact a template names that no longer
/// exist. Returns how many references went. Rows of a distribution rule are left: dropping one
/// would break the single remainder, and the rule is refused when it is used.
pub fn drop_dangling_references(doc: &mut VaultDoc) -> usize {
    let categories: Vec<String> = doc.categories.iter().map(|each| each.key.clone()).collect();
    let stores: Vec<String> = doc.stores.iter().map(|each| each.id.clone()).collect();
    let contacts: Vec<String> = doc.contacts.iter().map(|each| each.id.clone()).collect();
    let mut dropped = 0;
    for template in &mut doc.operation_templates {
        let before = template.categories.len();
        template.categories.retain(|key| categories.contains(key));
        dropped += before - template.categories.len();
        if template
            .store_id
            .as_ref()
            .is_some_and(|id| !stores.contains(id))
        {
            template.store_id = None;
            dropped += 1;
        }
        if template
            .contact_id
            .as_ref()
            .is_some_and(|id| !contacts.contains(id))
        {
            template.contact_id = None;
            dropped += 1;
        }
    }
    dropped
}
