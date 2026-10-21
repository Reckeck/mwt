//! Workflows made from the ready-made templates of the gallery.

use std::collections::BTreeMap;

use rust_decimal::Decimal;

use super::{SavedWorkflow, UnlockedVault, WorkflowDraft};
use crate::Amount;
use crate::error::{Error, Result};
use crate::workflow::templates::{
    TemplateDefaults, TemplateFieldKind, WorkflowTemplateInfo, build_template, template_list,
};

impl UnlockedVault {
    /// The gallery: the savings share and the budget currency come from the vault.
    pub fn workflow_templates(&self) -> Vec<WorkflowTemplateInfo> {
        let pct = (self.doc.settings.savings_rate * Amount::new(Decimal::ONE_HUNDRED))
            .round(2)
            .into_inner()
            .normalize();
        template_list(&TemplateDefaults {
            savings_pct: pct.to_string(),
            currency: self.budget_currency().to_lowercase(),
        })
    }

    /// Builds the template's graph from the form and saves it as an active workflow.
    pub fn workflow_from_template(
        &mut self,
        id: &str,
        params: &BTreeMap<String, String>,
        locale: &str,
        now: i64,
    ) -> Result<SavedWorkflow> {
        let info = self
            .workflow_templates()
            .into_iter()
            .find(|info| info.id == id)
            .ok_or_else(|| Error::Missing(format!("template {id}")))?;
        for field in &info.fields {
            let value = params.get(&field.key).map_or("", |value| value.trim());
            if value.is_empty() {
                if field.required {
                    return Err(Error::InvalidInput(format!("{}: required", field.key)));
                }
                continue;
            }
            let valid = match field.kind {
                TemplateFieldKind::Store => self.find_store(value).is_some(),
                TemplateFieldKind::Number => {
                    Amount::parse(value).is_ok_and(|number| number > Amount::ZERO)
                }
                TemplateFieldKind::Weekday => value.parse::<u8>().is_ok_and(|day| day <= 6),
                TemplateFieldKind::Day => {
                    value.parse::<u8>().is_ok_and(|day| (1..=31).contains(&day))
                }
                TemplateFieldKind::Category
                | TemplateFieldKind::Currency
                | TemplateFieldKind::Budget => true,
            };
            if !valid {
                return Err(Error::InvalidInput(format!(
                    "{}: invalid value {value}",
                    field.key
                )));
            }
        }
        let built = build_template(id, params, locale)
            .ok_or_else(|| Error::Missing(format!("template {id}")))?;
        let draft = WorkflowDraft {
            name: built.name,
            description: built.description,
            active: true,
            graph: built.graph,
            react_to_workflows: false,
            max_amount: None,
        };
        self.save_workflow(None, draft, now)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calendar::now_secs;
    use crate::model::store::StoreKind;
    use crate::service::test_support::*;

    fn params(pairs: &[(&str, &str)]) -> BTreeMap<String, String> {
        pairs
            .iter()
            .map(|(key, value)| (key.to_string(), value.to_string()))
            .collect()
    }

    #[test]
    fn defaults_come_from_the_vault() {
        let (_dir, vault) = empty_vault();
        let list = vault.workflow_templates();
        let pay = list
            .iter()
            .find(|info| info.id == "pay_yourself_first")
            .unwrap();
        let pct = pay.fields.iter().find(|field| field.key == "pct").unwrap();
        assert_eq!(pct.default, "10");
    }

    #[test]
    fn a_missing_or_unknown_account_is_refused() {
        let (_dir, mut vault) = empty_vault();
        let no_target = vault.workflow_from_template(
            "pay_yourself_first",
            &params(&[("category", "salary"), ("pct", "10")]),
            "ru",
            now_secs(),
        );
        assert!(matches!(no_target, Err(Error::InvalidInput(_))));
        let unknown = vault.workflow_from_template(
            "pay_yourself_first",
            &params(&[("category", "salary"), ("pct", "10"), ("target", "nope")]),
            "ru",
            now_secs(),
        );
        assert!(matches!(unknown, Err(Error::InvalidInput(_))));
    }

    #[test]
    fn round_up_collects_the_change_of_a_spend() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Cash, None);
        let jar = store(&mut vault, "Jar", StoreKind::Bank, None);
        vault
            .add_transaction(income(&card, "1000", "opening"))
            .unwrap();
        let saved = vault
            .workflow_from_template(
                "round_up",
                &params(&[
                    ("from", &card),
                    ("to", &jar),
                    ("step", "100"),
                    ("weekday", "6"),
                ]),
                "en",
                now_secs(),
            )
            .unwrap();
        assert!(saved.workflow.active);
        assert_eq!(saved.workflow.name, "Round-up jar");

        vault
            .add_transaction(payment(&card, "230", "shop"))
            .unwrap();
        let pass = vault.run_due_workflows(now_secs() + 5, "UTC").unwrap();
        assert_eq!(pass.runs.len(), 1, "{:?}", pass.runs);
        assert!(pass.created.is_empty(), "no money moves on a spend");
        let jar_value = vault
            .workflow_vars()
            .into_iter()
            .find(|var| var.name == "round_up")
            .map(|var| var.value)
            .unwrap();
        assert!(jar_value.contains(r#""amount":"70""#), "{jar_value}");
    }
}
