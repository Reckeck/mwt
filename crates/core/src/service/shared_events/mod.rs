//! Shared events («посиделки»): the list of what was bought together, who
//! shares what, and the debts that leaves. Shares, balances and statuses are
//! worked out from the list ([`split`]) and never stored. This file holds the
//! event itself, its people and its items; purchases, settlements, the
//! analytics hooks and the text for a chat live next to it.

use crate::Amount;
use crate::error::{Error, Result};
use crate::ipc_contract::{
    SetShareOverrideInput, SetSharedItemSplitInput, SharedEventDraft, SharedItemDraft,
    SharedParticipantDraft,
};
use crate::model::contact::{Contact, ContactAccount, ContactKind, DetailKind};
use crate::model::shared_event::{
    EventItem, GuestDetails, ItemSplit, ME, Participant, ROUNDING_STEPS, ShareOverride,
    SharedEvent, SharedEventStatus,
};
use crate::model::vault::now_iso;

use super::UnlockedVault;
use super::contacts::ContactDraft;

mod analytics;
mod purchases;
mod settle;
pub(crate) mod split;
#[cfg(test)]
mod testing;
mod text;
mod view;

pub use view::{
    ContactEventBalance, EventTotals, ItemShareView, ItemView, MyPosition, PersonKind, PersonView,
    PlannedPayment, PurchaseView, SettlementView, SharedEventSummary, SharedEventView,
    SharedTxLink,
};

/// The category a payment of the event gets when nobody chose one.
const FALLBACK_CATEGORY: &str = "other";

/// The category of an item's spending: its own, the event's, else «other».
pub(crate) fn item_category(event: &SharedEvent, item: &EventItem) -> String {
    item.category_id
        .clone()
        .or_else(|| event.category_id.clone())
        .unwrap_or_else(|| FALLBACK_CATEGORY.to_string())
}

/// Check a `YYYY-MM-DD` date.
fn parse_day(raw: &str) -> Result<chrono::NaiveDate> {
    chrono::NaiveDate::parse_from_str(raw.trim(), "%Y-%m-%d")
        .map_err(|_| Error::InvalidInput("a date is YYYY-MM-DD".into()))
}

/// The dates of an event, checked: the end is not before the start, and a
/// range needs its start. An end equal to the start is a single day.
fn check_dates(
    starts_on: Option<String>,
    ends_on: Option<String>,
) -> Result<(Option<String>, Option<String>)> {
    let clean = |raw: Option<String>| {
        raw.map(|day| day.trim().to_string())
            .filter(|day| !day.is_empty())
    };
    let (starts_on, ends_on) = (clean(starts_on), clean(ends_on));
    let start = starts_on.as_deref().map(parse_day).transpose()?;
    let end = ends_on.as_deref().map(parse_day).transpose()?;
    match (start, end) {
        (None, Some(_)) => Err(Error::InvalidInput(
            "the end date needs a start date".into(),
        )),
        (Some(first), Some(last)) if last < first => Err(Error::InvalidInput(
            "the event cannot end before it starts".into(),
        )),
        (Some(first), Some(last)) if last == first => Ok((starts_on, None)),
        _ => Ok((starts_on, ends_on)),
    }
}

fn check_step(step: Option<Amount>) -> Result<Amount> {
    let step = step.unwrap_or_else(split::one);
    if ROUNDING_STEPS
        .iter()
        .any(|allowed| Amount::parse(allowed).is_ok_and(|value| value == step))
    {
        Ok(step)
    } else {
        Err(Error::InvalidInput(
            "the rounding step is 0.01, 1, 10, 50 or 100".into(),
        ))
    }
}

/// Whether an item split names only people of the event.
fn clean_details(details: Option<GuestDetails>) -> Result<Option<GuestDetails>> {
    let Some(mut details) = details else {
        return Ok(None);
    };
    details.phone = details.phone.trim().to_string();
    details.messenger = details.messenger.trim().to_string();
    details.note = details.note.trim().to_string();
    details.requisites = UnlockedVault::clean_accounts(details.requisites)?;
    Ok((!details.is_empty()).then_some(details))
}

impl UnlockedVault {
    fn check_shared_category(&self, category: Option<&str>) -> Result<Option<String>> {
        let Some(key) = category.map(str::trim).filter(|key| !key.is_empty()) else {
            return Ok(None);
        };
        if self.doc.categories.iter().any(|known| known.key == key) {
            Ok(Some(key.to_string()))
        } else {
            Err(Error::Missing(format!("category {key}")))
        }
    }

    /// A participant from a draft, checked against the people already there.
    fn build_participant(
        &self,
        draft: &SharedParticipantDraft,
        others: &[Participant],
    ) -> Result<Participant> {
        let id = match draft
            .id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            Some(id) if id == ME || others.iter().any(|person| person.id == id) => {
                return Err(Error::InvalidInput(format!("participant id {id} is taken")));
            }
            Some(id) => id.to_string(),
            None => uuid::Uuid::new_v4().to_string(),
        };
        if let Some(contact_id) = draft.contact_id.as_deref().filter(|id| !id.is_empty()) {
            let contact = self
                .doc
                .contacts
                .iter()
                .find(|contact| contact.id == contact_id)
                .ok_or_else(|| Error::Missing(format!("contact {contact_id}")))?;
            if others
                .iter()
                .any(|person| person.contact_id.as_deref() == Some(contact_id))
            {
                return Err(Error::InvalidInput(
                    "this contact already takes part".into(),
                ));
            }
            return Ok(Participant {
                id,
                name: contact.name.clone(),
                contact_id: Some(contact.id.clone()),
                details: None,
            });
        }
        let name = draft.name.trim().to_string();
        if name.is_empty() {
            return Err(Error::InvalidInput("a guest needs a name".into()));
        }
        Ok(Participant {
            id,
            name,
            contact_id: None,
            details: clean_details(draft.details.clone())?,
        })
    }

    fn check_organizer(organizer: &str, participants: &[Participant]) -> Result<String> {
        let organizer = organizer.trim();
        if organizer.is_empty() || organizer == ME {
            return Ok(ME.to_string());
        }
        if participants.iter().any(|person| person.id == organizer) {
            Ok(organizer.to_string())
        } else {
            Err(Error::Missing(format!("participant {organizer}")))
        }
    }

    fn shared_currency(&self, raw: &str) -> String {
        let code = raw.trim().to_lowercase();
        if code.is_empty() {
            self.base_and_fx().0
        } else {
            code
        }
    }

    pub fn create_shared_event(&mut self, draft: SharedEventDraft) -> Result<SharedEventView> {
        let title = draft.title.trim().to_string();
        if title.is_empty() {
            return Err(Error::InvalidInput("an event needs a title".into()));
        }
        let (starts_on, ends_on) = check_dates(draft.starts_on, draft.ends_on)?;
        let rounding_step = check_step(draft.rounding_step)?;
        let category_id = self.check_shared_category(draft.category_id.as_deref())?;
        let mut participants: Vec<Participant> = Vec::new();
        for person in &draft.participants {
            let built = self.build_participant(person, &participants)?;
            participants.push(built);
        }
        let organizer = Self::check_organizer(&draft.organizer, &participants)?;
        let event = SharedEvent {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            created_at: now_iso(),
            note: draft.note.trim().to_string(),
            archived: false,
            starts_on,
            ends_on,
            currency: self.shared_currency(&draft.currency),
            rounding_step,
            category_id,
            organizer,
            i_participate: draft.i_participate,
            participants,
            items: Vec::new(),
            settlements: Vec::new(),
            adjustments: Vec::new(),
        };
        let id = event.id.clone();
        self.doc.shared_events.push(event);
        self.persist()?;
        self.shared_event_view(&id)
    }

    /// Change the event's own fields. Things the purchases and settlements
    /// already depend on stay put: the currency, who organizes, whether I take
    /// part (while it would strand my payments).
    pub fn update_shared_event(
        &mut self,
        id: &str,
        draft: SharedEventDraft,
    ) -> Result<SharedEventView> {
        let title = draft.title.trim().to_string();
        if title.is_empty() {
            return Err(Error::InvalidInput("an event needs a title".into()));
        }
        let (starts_on, ends_on) = check_dates(draft.starts_on, draft.ends_on)?;
        let rounding_step = check_step(draft.rounding_step)?;
        let category_id = self.check_shared_category(draft.category_id.as_deref())?;
        let currency = self.shared_currency(&draft.currency);
        let event = self.find_shared_event(id)?;
        let organizer = if draft.organizer.trim().is_empty() {
            event.organizer.clone()
        } else {
            Self::check_organizer(&draft.organizer, &event.participants)?
        };
        let has_purchases = event.items.iter().any(|item| !item.purchases.is_empty());
        if currency != event.currency && has_purchases {
            return Err(Error::InvalidInput(
                "the currency cannot change once something is bought".into(),
            ));
        }
        if organizer != event.organizer && !event.settlements.is_empty() {
            return Err(Error::InvalidInput(
                "the organizer cannot change once people have settled".into(),
            ));
        }
        if !draft.i_participate && event.i_participate {
            let in_my_money = event
                .items
                .iter()
                .flat_map(|item| &item.purchases)
                .any(|purchase| purchase.buyer == ME)
                || event
                    .settlements
                    .iter()
                    .any(|settlement| settlement.participant_id == ME || event.organizer == ME);
            if in_my_money {
                return Err(Error::InvalidInput(
                    "you paid or settled in this event: you cannot leave it".into(),
                ));
            }
        }
        let event = self.find_shared_event_mut(id)?;
        event.title = title;
        event.note = draft.note.trim().to_string();
        event.starts_on = starts_on;
        event.ends_on = ends_on;
        event.currency = currency;
        event.rounding_step = rounding_step;
        event.category_id = category_id;
        event.organizer = organizer;
        event.i_participate = draft.i_participate;
        if !event.i_participate {
            // The owner is not a sharer any more.
            let me = ME.to_string();
            for item in &mut event.items {
                match &mut item.split {
                    ItemSplit::Equal { among } => among.retain(|who| *who != me),
                    ItemSplit::ByUnits { units } => {
                        units.remove(ME);
                    }
                }
            }
            event.adjustments.retain(|fix| fix.participant_id != me);
        }
        self.persist()?;
        self.shared_event_view(id)
    }

    /// Delete an event with the payments it made; payments that were only
    /// linked stay, unlinked.
    pub fn delete_shared_event(&mut self, id: &str) -> Result<()> {
        let event = self.find_shared_event(id)?.clone();
        let mut owned: Vec<String> = Vec::new();
        // Payments that were only adopted stay where they are.
        for purchase in event.items.iter().flat_map(|item| &item.purchases) {
            if let (Some(tx_id), true) = (&purchase.tx_id, purchase.tx_owned) {
                owned.push(tx_id.clone());
            }
        }
        owned.extend(
            event
                .settlements
                .iter()
                .filter_map(|settlement| settlement.tx_id.clone()),
        );
        self.doc
            .transactions
            .retain(|tx| !owned.contains(&tx.meta().id));
        for tx_id in &owned {
            self.set_property_link(tx_id, None);
        }
        self.doc.shared_events.retain(|known| known.id != id);
        self.recompute_holdings();
        self.persist()
    }

    /// Put a settled event away (or take it back).
    pub fn archive_shared_event(&mut self, id: &str, archived: bool) -> Result<()> {
        if archived {
            let view = self.shared_event_view(id)?;
            if view.status != SharedEventStatus::Closed
                && view.status != SharedEventStatus::Planning
            {
                return Err(Error::InvalidInput(
                    "only a settled event can be archived".into(),
                ));
            }
        }
        self.find_shared_event_mut(id)?.archived = archived;
        self.persist()
    }

    // -- people ---------------------------------------------------------------

    pub fn add_shared_participant(
        &mut self,
        event_id: &str,
        draft: SharedParticipantDraft,
    ) -> Result<SharedEventView> {
        let participant = {
            let event = self.find_shared_event(event_id)?;
            self.build_participant(&draft, &event.participants)?
        };
        self.find_shared_event_mut(event_id)?
            .participants
            .push(participant);
        self.persist()?;
        self.shared_event_view(event_id)
    }

    /// Rename a guest, change their details, or link them to a contact
    /// (their debts then show in the contact's card).
    pub fn update_shared_participant(
        &mut self,
        event_id: &str,
        participant_id: &str,
        draft: SharedParticipantDraft,
    ) -> Result<SharedEventView> {
        let event = self.find_shared_event(event_id)?;
        let others: Vec<Participant> = event
            .participants
            .iter()
            .filter(|person| person.id != participant_id)
            .cloned()
            .collect();
        if !event
            .participants
            .iter()
            .any(|person| person.id == participant_id)
        {
            return Err(Error::Missing(format!("participant {participant_id}")));
        }
        let mut rebuilt =
            self.build_participant(&SharedParticipantDraft { id: None, ..draft }, &others)?;
        rebuilt.id = participant_id.to_string();
        let event = self.find_shared_event_mut(event_id)?;
        if let Some(person) = event
            .participants
            .iter_mut()
            .find(|person| person.id == participant_id)
        {
            *person = rebuilt;
        }
        self.persist()?;
        self.shared_event_view(event_id)
    }

    /// Take someone out of the event; refused once money or a hand-set total
    /// mentions them.
    pub fn remove_shared_participant(
        &mut self,
        event_id: &str,
        participant_id: &str,
    ) -> Result<SharedEventView> {
        let event = self.find_shared_event_mut(event_id)?;
        if !event
            .participants
            .iter()
            .any(|person| person.id == participant_id)
        {
            return Err(Error::Missing(format!("participant {participant_id}")));
        }
        let busy = event.organizer == participant_id
            || event
                .items
                .iter()
                .flat_map(|item| &item.purchases)
                .any(|purchase| purchase.buyer == participant_id)
            || event.settlements.iter().any(|settlement| {
                settlement.participant_id == participant_id
                    || settlement.to.iter().any(|who| who == participant_id)
            })
            || event
                .adjustments
                .iter()
                .any(|fix| fix.participant_id == participant_id);
        if busy {
            return Err(Error::InvalidInput(
                "this person organizes, paid, settled or has a fixed total in the event".into(),
            ));
        }
        event
            .participants
            .retain(|person| person.id != participant_id);
        for item in &mut event.items {
            match &mut item.split {
                ItemSplit::Equal { among } => among.retain(|who| who != participant_id),
                ItemSplit::ByUnits { units } => {
                    units.remove(participant_id);
                }
            }
        }
        self.persist()?;
        self.shared_event_view(event_id)
    }

    /// «Сделать контактом»: a guest becomes a contact of the directory with
    /// the details they had, and is linked to it.
    pub fn make_shared_participant_contact(
        &mut self,
        event_id: &str,
        participant_id: &str,
    ) -> Result<Contact> {
        let guest = self
            .find_shared_event(event_id)?
            .participants
            .iter()
            .find(|person| person.id == participant_id)
            .cloned()
            .ok_or_else(|| Error::Missing(format!("participant {participant_id}")))?;
        if guest.contact_id.is_some() {
            return Err(Error::InvalidInput(
                "this person is a contact already".into(),
            ));
        }
        let details = guest.details.clone().unwrap_or_default();
        let mut accounts: Vec<ContactAccount> = Vec::new();
        if !details.phone.is_empty() {
            accounts.push(ContactAccount {
                label: "Телефон".into(),
                kind: DetailKind::Phone,
                value: details.phone.clone(),
                ..ContactAccount::default()
            });
        }
        if !details.messenger.is_empty() {
            accounts.push(ContactAccount {
                label: "Мессенджер".into(),
                kind: DetailKind::Other,
                value: details.messenger.clone(),
                ..ContactAccount::default()
            });
        }
        accounts.extend(
            details
                .requisites
                .iter()
                .cloned()
                .map(|account| ContactAccount {
                    // Fresh ids: the contact owns its accounts.
                    id: String::new(),
                    ..account
                }),
        );
        let contact = self.create_contact(ContactDraft {
            name: guest.name.clone(),
            kind: ContactKind::Person,
            note: details.note.clone(),
            accounts,
            group: String::new(),
        })?;
        if let Some(person) = self
            .find_shared_event_mut(event_id)?
            .participants
            .iter_mut()
            .find(|person| person.id == participant_id)
        {
            person.contact_id = Some(contact.id.clone());
            person.details = None;
        }
        self.persist()?;
        Ok(contact)
    }

    /// Fix a person's total by hand (the rest is spread over the others), or
    /// take the fixed total back with `None`.
    pub fn set_shared_share_override(
        &mut self,
        input: SetShareOverrideInput,
    ) -> Result<SharedEventView> {
        let event = self.find_shared_event_mut(&input.event_id)?;
        let present = (input.participant_id == ME && event.i_participate)
            || event
                .participants
                .iter()
                .any(|person| person.id == input.participant_id);
        if !present {
            return Err(Error::Missing(format!(
                "participant {}",
                input.participant_id
            )));
        }
        event
            .adjustments
            .retain(|fix| fix.participant_id != input.participant_id);
        if let Some(total) = input.total {
            if total.is_negative() {
                return Err(Error::InvalidInput("a total cannot be negative".into()));
            }
            event.adjustments.push(ShareOverride {
                participant_id: input.participant_id,
                total,
            });
        }
        self.persist()?;
        self.shared_event_view(&input.event_id)
    }

    // -- items ----------------------------------------------------------------

    fn clean_item(&self, draft: &SharedItemDraft) -> Result<(String, String, Option<String>)> {
        let title = draft.title.trim().to_string();
        if title.is_empty() {
            return Err(Error::InvalidInput("an item needs a title".into()));
        }
        let unit = match draft.unit.trim() {
            "" => "шт".to_string(),
            unit => unit.to_string(),
        };
        Ok((
            title,
            unit,
            self.check_shared_category(draft.category_id.as_deref())?,
        ))
    }

    fn check_plan(draft: &SharedItemDraft) -> Result<()> {
        if let Some(plan) = &draft.plan
            && (!plan.qty.is_positive() || plan.price.is_negative())
        {
            return Err(Error::InvalidInput(
                "a plan needs a positive quantity and a price".into(),
            ));
        }
        Ok(())
    }

    pub fn add_shared_item(
        &mut self,
        event_id: &str,
        draft: SharedItemDraft,
    ) -> Result<SharedEventView> {
        self.find_shared_event(event_id)?;
        let (title, unit, category_id) = self.clean_item(&draft)?;
        Self::check_plan(&draft)?;
        self.find_shared_event_mut(event_id)?.items.push(EventItem {
            id: uuid::Uuid::new_v4().to_string(),
            title,
            unit,
            category_id,
            plan: draft.plan,
            purchases: Vec::new(),
            cancelled: draft.cancelled,
            split: ItemSplit::default(),
        });
        self.persist()?;
        self.shared_event_view(event_id)
    }

    pub fn update_shared_item(
        &mut self,
        event_id: &str,
        item_id: &str,
        draft: SharedItemDraft,
    ) -> Result<SharedEventView> {
        let (title, unit, category_id) = self.clean_item(&draft)?;
        Self::check_plan(&draft)?;
        let event = self.find_shared_event_mut(event_id)?;
        let item = event
            .items
            .iter_mut()
            .find(|item| item.id == item_id)
            .ok_or_else(|| Error::Missing(format!("item {item_id}")))?;
        if draft.cancelled && !item.purchases.is_empty() {
            return Err(Error::InvalidInput(
                "an item with purchases cannot be cancelled".into(),
            ));
        }
        item.title = title;
        item.unit = unit;
        item.category_id = category_id;
        item.plan = draft.plan;
        item.cancelled = draft.cancelled;
        self.persist()?;
        self.shared_event_view(event_id)
    }

    /// Delete an item nothing was bought for.
    pub fn delete_shared_item(&mut self, event_id: &str, item_id: &str) -> Result<SharedEventView> {
        let event = self.find_shared_event_mut(event_id)?;
        let item = event
            .items
            .iter()
            .find(|item| item.id == item_id)
            .ok_or_else(|| Error::Missing(format!("item {item_id}")))?;
        if !item.purchases.is_empty() {
            return Err(Error::InvalidInput(
                "delete the purchases of this item first".into(),
            ));
        }
        event.items.retain(|item| item.id != item_id);
        self.persist()?;
        self.shared_event_view(event_id)
    }

    /// Who shares an item and how.
    pub fn set_shared_item_split(
        &mut self,
        input: SetSharedItemSplitInput,
    ) -> Result<SharedEventView> {
        let event = self.find_shared_event_mut(&input.event_id)?;
        let known = |who: &String, event: &SharedEvent| {
            (*who == ME && event.i_participate)
                || event.participants.iter().any(|person| &person.id == who)
        };
        let split = match input.split {
            ItemSplit::Equal { among } => {
                if let Some(unknown) = among.iter().find(|who| !known(who, event)) {
                    return Err(Error::Missing(format!("participant {unknown}")));
                }
                ItemSplit::Equal { among }
            }
            ItemSplit::ByUnits { units } => {
                if let Some(unknown) = units.keys().find(|who| !known(who, event)) {
                    return Err(Error::Missing(format!("participant {unknown}")));
                }
                if units.values().any(|qty| qty.is_negative()) {
                    return Err(Error::InvalidInput("units cannot be negative".into()));
                }
                ItemSplit::ByUnits { units }
            }
        };
        event
            .items
            .iter_mut()
            .find(|item| item.id == input.item_id)
            .ok_or_else(|| Error::Missing(format!("item {}", input.item_id)))?
            .split = split;
        self.persist()?;
        self.shared_event_view(&input.event_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::vault::VaultDoc;
    use crate::service::test_support::*;
    use crate::storage::{SnapshotFormat, VaultFile, decode, encode};

    fn draft(title: &str) -> SharedEventDraft {
        SharedEventDraft {
            title: title.into(),
            currency: "rub".into(),
            ..SharedEventDraft::default()
        }
    }

    fn guest(name: &str) -> SharedParticipantDraft {
        SharedParticipantDraft {
            name: name.into(),
            ..SharedParticipantDraft::default()
        }
    }

    #[test]
    fn an_event_is_created_renamed_and_deleted() {
        let (_dir, mut vault) = empty_vault();
        let view = vault
            .create_shared_event(SharedEventDraft {
                starts_on: Some("2026-11-02".into()),
                ends_on: Some("2026-11-04".into()),
                participants: vec![guest("Оля")],
                ..draft("  Шашлыки ")
            })
            .unwrap();
        assert_eq!(view.event.title, "Шашлыки");
        assert_eq!(view.event.currency, "rub");
        assert_eq!(view.event.organizer, ME);
        assert!(view.event.i_participate);
        assert_eq!(view.event.rounding_step, amount("1"));
        assert_eq!(view.status, SharedEventStatus::Planning);
        // me and the guest
        assert_eq!(view.people.len(), 2);

        let id = view.event.id;
        let renamed = vault
            .update_shared_event(
                &id,
                SharedEventDraft {
                    starts_on: Some("2026-11-02".into()),
                    ends_on: Some("2026-11-02".into()),
                    ..draft("Мангал")
                },
            )
            .unwrap();
        assert_eq!(renamed.event.title, "Мангал");
        // an end equal to the start is one day
        assert_eq!(renamed.event.ends_on, None);
        assert_eq!(vault.list_shared_events(false).len(), 1);
        vault.delete_shared_event(&id).unwrap();
        assert!(vault.list_shared_events(true).is_empty());
        assert!(vault.delete_shared_event(&id).is_err());
    }

    #[test]
    fn dates_are_checked() {
        let (_dir, mut vault) = empty_vault();
        let bad = |starts: Option<&str>, ends: Option<&str>| SharedEventDraft {
            starts_on: starts.map(str::to_string),
            ends_on: ends.map(str::to_string),
            ..draft("Trip")
        };
        let before = vault
            .create_shared_event(bad(Some("2026-11-04"), Some("2026-11-02")))
            .unwrap_err();
        assert_eq!(before.code(), "INVALID_INPUT");
        assert!(
            vault
                .create_shared_event(bad(None, Some("2026-11-02")))
                .is_err()
        );
        assert!(
            vault
                .create_shared_event(bad(Some("2.11.2026"), None))
                .is_err()
        );
        assert!(vault.doc.shared_events.is_empty());
        // dates are optional
        assert!(vault.create_shared_event(bad(None, None)).is_ok());
    }

    #[test]
    fn the_list_is_ordered_by_the_event_date() {
        let (_dir, mut vault) = empty_vault();
        for (title, day) in [
            ("A", Some("2026-11-02")),
            ("B", Some("2026-12-01")),
            ("C", None),
        ] {
            vault
                .create_shared_event(SharedEventDraft {
                    starts_on: day.map(str::to_string),
                    ..draft(title)
                })
                .unwrap();
        }
        // «C» has no date: it sorts by the day it was made
        vault.doc.shared_events[2].created_at = "2026-11-15T10:00:00Z".into();
        let titles: Vec<String> = vault
            .list_shared_events(false)
            .into_iter()
            .map(|row| row.title)
            .collect();
        assert_eq!(titles, ["B", "C", "A"]);
    }

    #[test]
    fn participants_are_contacts_or_guests() {
        let (_dir, mut vault) = empty_vault();
        let petya = vault
            .create_contact(ContactDraft {
                name: "Петя".into(),
                ..ContactDraft::default()
            })
            .unwrap();
        let view = vault.create_shared_event(draft("Event")).unwrap();
        let id = view.event.id;
        vault
            .add_shared_participant(
                &id,
                SharedParticipantDraft {
                    contact_id: Some(petya.id.clone()),
                    ..SharedParticipantDraft::default()
                },
            )
            .unwrap();
        // the same contact twice, and a nameless guest
        assert!(
            vault
                .add_shared_participant(
                    &id,
                    SharedParticipantDraft {
                        contact_id: Some(petya.id.clone()),
                        ..SharedParticipantDraft::default()
                    },
                )
                .is_err()
        );
        assert!(vault.add_shared_participant(&id, guest("  ")).is_err());
        let view = vault.add_shared_participant(&id, guest("Оля")).unwrap();
        let kinds: Vec<PersonKind> = view.people.iter().map(|row| row.kind).collect();
        assert_eq!(
            kinds,
            [PersonKind::Me, PersonKind::Contact, PersonKind::Guest]
        );
        assert_eq!(view.people[1].name, "Петя");
    }

    #[test]
    fn a_guest_with_details_becomes_a_contact_with_them() {
        let (_dir, mut vault) = empty_vault();
        let view = vault.create_shared_event(draft("Event")).unwrap();
        let id = view.event.id;
        let view = vault
            .add_shared_participant(
                &id,
                SharedParticipantDraft {
                    name: "Оля".into(),
                    details: Some(GuestDetails {
                        phone: "+7 912 345-67-89".into(),
                        messenger: "@olya".into(),
                        requisites: vec![ContactAccount {
                            label: "Карта".into(),
                            kind: DetailKind::Card,
                            value: "2200 0000 0000 4411".into(),
                            ..ContactAccount::default()
                        }],
                        note: "с работы".into(),
                    }),
                    ..SharedParticipantDraft::default()
                },
            )
            .unwrap();
        let guest_id = view.people[1].id.clone();
        assert!(view.people[1].has_details);
        let contact = vault
            .make_shared_participant_contact(&id, &guest_id)
            .unwrap();
        assert_eq!(contact.name, "Оля");
        assert_eq!(contact.note, "с работы");
        assert_eq!(contact.accounts.len(), 3);
        assert!(contact.accounts.iter().all(|account| account.active));
        assert!(
            contact
                .accounts
                .iter()
                .any(|account| account.kind == DetailKind::Card)
        );
        let view = vault.shared_event_view(&id).unwrap();
        assert_eq!(view.people[1].kind, PersonKind::Contact);
        assert_eq!(
            view.people[1].contact_id.as_deref(),
            Some(contact.id.as_str())
        );
        assert!(!view.people[1].has_details);
        // already a contact
        assert!(
            vault
                .make_shared_participant_contact(&id, &guest_id)
                .is_err()
        );
    }

    #[test]
    fn a_person_with_money_in_the_event_cannot_be_removed() {
        let (_dir, mut vault) = empty_vault();
        let view = vault
            .create_shared_event(SharedEventDraft {
                participants: vec![guest("Оля"), guest("Вася")],
                organizer: String::new(),
                ..draft("Event")
            })
            .unwrap();
        let id = view.event.id.clone();
        let olya = view.people[1].id.clone();
        let vasya = view.people[2].id.clone();
        vault
            .add_shared_item(
                &id,
                SharedItemDraft {
                    title: "Хлеб".into(),
                    ..SharedItemDraft::default()
                },
            )
            .unwrap();
        vault.find_shared_event_mut(&id).unwrap().items[0]
            .purchases
            .push(crate::model::shared_event::Purchase {
                id: "p".into(),
                buyer: vasya.clone(),
                qty: amount("1"),
                amount: amount("100"),
                currency: "rub".into(),
                price_mode: crate::model::shared_event::PriceMode::Total,
                rate_to_event: amount("1"),
                ..Default::default()
            });
        assert!(vault.remove_shared_participant(&id, &vasya).is_err());
        vault.remove_shared_participant(&id, &olya).unwrap();
    }

    #[test]
    fn items_are_planned_cancelled_and_deleted() {
        let (_dir, mut vault) = empty_vault();
        let id = vault.create_shared_event(draft("Event")).unwrap().event.id;
        let plan = crate::model::shared_event::ItemPlan {
            qty: amount("6"),
            price: amount("200"),
            price_mode: crate::model::shared_event::PriceMode::PerUnit,
        };
        let view = vault
            .add_shared_item(
                &id,
                SharedItemDraft {
                    title: "Пиво".into(),
                    plan: Some(plan.clone()),
                    ..SharedItemDraft::default()
                },
            )
            .unwrap();
        assert_eq!(view.totals.planned, amount("1200"));
        assert_eq!(view.event.items[0].unit, "шт");
        let item_id = view.event.items[0].id.clone();
        let view = vault
            .update_shared_item(
                &id,
                &item_id,
                SharedItemDraft {
                    title: "Пиво".into(),
                    cancelled: true,
                    plan: Some(plan),
                    ..SharedItemDraft::default()
                },
            )
            .unwrap();
        // a cancelled item is out of the totals
        assert_eq!(view.totals.planned, amount("0"));
        assert!(
            vault
                .add_shared_item(&id, SharedItemDraft::default())
                .is_err()
        );
        vault.delete_shared_item(&id, &item_id).unwrap();
        assert!(vault.shared_event_view(&id).unwrap().event.items.is_empty());
    }

    #[test]
    fn a_split_names_only_people_of_the_event() {
        let (_dir, mut vault) = empty_vault();
        let view = vault
            .create_shared_event(SharedEventDraft {
                participants: vec![guest("Оля")],
                ..draft("Event")
            })
            .unwrap();
        let id = view.event.id.clone();
        let olya = view.people[1].id.clone();
        let item_id = vault
            .add_shared_item(
                &id,
                SharedItemDraft {
                    title: "Пиво".into(),
                    ..SharedItemDraft::default()
                },
            )
            .unwrap()
            .event
            .items[0]
            .id
            .clone();
        let set = |vault: &mut UnlockedVault, split: ItemSplit| {
            vault.set_shared_item_split(SetSharedItemSplitInput {
                event_id: id.clone(),
                item_id: item_id.clone(),
                split,
            })
        };
        assert!(
            set(
                &mut vault,
                ItemSplit::Equal {
                    among: vec!["ghost".into()]
                }
            )
            .is_err()
        );
        set(
            &mut vault,
            ItemSplit::Equal {
                among: vec![olya.clone()],
            },
        )
        .unwrap();
        let units = std::collections::BTreeMap::from([(olya, amount("3"))]);
        set(&mut vault, ItemSplit::ByUnits { units }).unwrap();
    }

    /// The vault of a build before shared events: the document without the field.
    fn old_document_json() -> Vec<u8> {
        let mut value = serde_json::to_value(VaultDoc::default()).unwrap();
        value.as_object_mut().unwrap().remove("shared_events");
        value["transactions"] = serde_json::json!([]);
        serde_json::to_vec(&value).unwrap()
    }

    #[test]
    fn an_old_vault_without_shared_events_opens_keeps_its_data_and_reads_again() {
        for fmt in [SnapshotFormat::Json, SnapshotFormat::Pack] {
            let json: serde_json::Value = serde_json::from_slice(&old_document_json()).unwrap();
            assert!(json.get("shared_events").is_none());
            let old_bytes = match fmt {
                SnapshotFormat::Json => old_document_json(),
                _ => rmp_serde::to_vec_named(&json).unwrap(),
            };
            let doc = decode(&old_bytes, fmt).unwrap();
            assert!(doc.shared_events.is_empty());
            assert_eq!(doc.version, crate::VAULT_FORMAT_VERSION);

            // through a real file: write, open, add an event, save, open again
            let dir = tempfile::tempdir().unwrap();
            let path = dir.path().join("old.wallet");
            VaultFile::save(&path, "pw", &doc, fmt).unwrap();
            let mut vault = UnlockedVault::open(path.clone(), "pw").unwrap();
            let stores = vault.doc.stores.len();
            let id = vault.create_shared_event(draft("После")).unwrap().event.id;
            drop(vault);
            let reopened = UnlockedVault::open(path, "pw").unwrap();
            assert_eq!(reopened.doc.stores.len(), stores);
            assert_eq!(reopened.doc.shared_events.len(), 1);
            assert_eq!(reopened.doc.shared_events[0].id, id);
            // and the encoded form round-trips
            let bytes = encode(&reopened.doc, fmt).unwrap();
            assert_eq!(decode(&bytes, fmt).unwrap().shared_events.len(), 1);
        }
    }
}
