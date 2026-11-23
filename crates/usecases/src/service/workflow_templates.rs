//! Shim of the `workflow_templates` area (spec core-strangler, F2, D1): the
//! gallery and the workflow made from a template live in
//! `crate::usecases::workflow::records`.

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use crate::calendar::now_secs;
    use crate::error::Error;
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
        let list = crate::usecases::workflow::records::workflow_templates(&vault.books());
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
        let no_target = crate::usecases::workflow::records::workflow_from_template(
            &mut vault,
            "pay_yourself_first",
            &params(&[("category", "salary"), ("pct", "10")]),
            "ru",
            now_secs(),
        );
        assert!(matches!(no_target, Err(Error::InvalidInput(_))));
        let unknown = crate::usecases::workflow::records::workflow_from_template(
            &mut vault,
            "pay_yourself_first",
            &params(&[("category", "salary"), ("pct", "10"), ("target", "nope")]),
            "ru",
            now_secs(),
        );
        assert!(matches!(unknown, Err(Error::InvalidInput(_))));
    }

    #[test]
    fn pay_yourself_first_listens_only_to_its_source_wallet() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Bank, None);
        let other = store(&mut vault, "Other", StoreKind::Bank, None);
        let cash = store(&mut vault, "Cash", StoreKind::Cash, None);
        crate::usecases::workflow::records::workflow_from_template(
            &mut vault,
            "pay_yourself_first",
            &params(&[
                ("category", "salary"),
                ("pct", "10"),
                ("target", &cash),
                ("source", &card),
            ]),
            "en",
            now_secs(),
        )
        .unwrap();

        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income(&other, "1000", "side"),
        )
        .unwrap();
        let pass =
            crate::usecases::workflow::runs::run_due_workflows(&mut vault, now_secs() + 5, "UTC")
                .unwrap();
        assert!(pass.runs.is_empty(), "{:?}", pass.runs);

        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income(&card, "1000", "salary"),
        )
        .unwrap();
        let pass =
            crate::usecases::workflow::runs::run_due_workflows(&mut vault, now_secs() + 10, "UTC")
                .unwrap();
        assert_eq!(pass.runs.len(), 1, "{:?}", pass.runs);
    }

    #[test]
    fn round_up_collects_the_change_of_a_spend() {
        let (_dir, mut vault) = empty_vault();
        let card = store(&mut vault, "Card", StoreKind::Cash, None);
        let jar = store(&mut vault, "Jar", StoreKind::Bank, None);
        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            income(&card, "1000", "opening"),
        )
        .unwrap();
        let saved = crate::usecases::workflow::records::workflow_from_template(
            &mut vault,
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

        crate::usecases::money::transactions::add_transaction(
            &mut vault,
            payment(&card, "230", "shop"),
        )
        .unwrap();
        let pass =
            crate::usecases::workflow::runs::run_due_workflows(&mut vault, now_secs() + 5, "UTC")
                .unwrap();
        assert_eq!(pass.runs.len(), 1, "{:?}", pass.runs);
        assert!(pass.created.is_empty(), "no money moves on a spend");
        let jar_value = crate::workflow::service::vars::variables(&vault.doc)
            .into_iter()
            .find(|var| var.name == "round_up")
            .map(|var| var.value)
            .unwrap();
        assert!(jar_value.contains(r#""amount":"70""#), "{jar_value}");
    }
}
