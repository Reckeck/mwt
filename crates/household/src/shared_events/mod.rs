//! Shared events ("shared events"): the list of what was bought together, who
//! shares what, and the debts that leaves. Shares, balances and statuses are
//! worked out from the list ([`split`]) and never stored.
//!
//! Pure part of the event, its people and its items: every `plan_*` checks a
//! request against a document and returns the event as it should become (or
//! the new record); nothing here writes. The writes are the scenarios in
//! `usecases::household::shared_events`.

use self::inputs::{
    SetShareOverrideInput, SetSharedItemSplitInput, SharedEventDraft, SharedItemDraft,
    SharedParticipantDraft,
};
use mwt_base::Amount;
use mwt_base::error::{Error, Result};
use mwt_base::time::unix_secs;
use mwt_model::VaultDoc;
use mwt_model::{Contact, ContactAccount, ContactKind, DetailKind};
use mwt_model::{
    EventItem, GuestDetails, ItemSplit, ME, Participant, ROUNDING_STEPS, ShareOverride,
    SharedEvent, SharedEventStatus,
};

pub mod analytics;
pub mod inputs;
pub mod ledger;
pub mod purchases;
pub mod settle;
pub mod split;
#[cfg(test)]
mod split_props;
pub mod text;
pub mod view;

#[cfg(test)]
mod split_tests;
pub use mwt_money::shared_split::item_category;

/// An instant for a date or an RFC3339 time typed by the user.
pub fn normalize_moment(raw: &str) -> Result<String> {
    let raw = raw.trim();
    if let Ok(day) = chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d") {
        return Ok(format!("{day}T12:00:00Z"));
    }
    if unix_secs(raw).is_some() {
        return Ok(raw.to_string());
    }
    Err(Error::InvalidInput(
        "a date is YYYY-MM-DD or an RFC3339 time".into(),
    ))
}

/// The event of the document.
pub fn find_event<'doc>(doc: &'doc VaultDoc, id: &str) -> Result<&'doc SharedEvent> {
    doc.shared_events
        .iter()
        .find(|event| event.id == id)
        .ok_or_else(|| Error::Missing(format!("shared event {id}")))
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

/// The guest's details, trimmed; `None` when nothing is left.
fn clean_details(details: Option<GuestDetails>) -> Result<Option<GuestDetails>> {
    let Some(mut details) = details else {
        return Ok(None);
    };
    details.phone = details.phone.trim().to_string();
    details.messenger = details.messenger.trim().to_string();
    details.note = details.note.trim().to_string();
    details.requisites = crate::contacts::clean_accounts(details.requisites)?;
    Ok((!details.is_empty()).then_some(details))
}

fn check_category(doc: &VaultDoc, category: Option<&str>) -> Result<Option<String>> {
    let Some(key) = category.map(str::trim).filter(|key| !key.is_empty()) else {
        return Ok(None);
    };
    if doc.categories.iter().any(|known| known.key == key) {
        Ok(Some(key.to_string()))
    } else {
        Err(Error::Missing(format!("category {key}")))
    }
}

/// A participant from a draft, checked against the people already there.
fn build_participant(
    doc: &VaultDoc,
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
        let contact = doc
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

/// The currency of an event: the one typed, else the base currency.
fn event_currency(base: &str, raw: &str) -> String {
    let code = raw.trim().to_lowercase();
    if code.is_empty() {
        base.to_string()
    } else {
        code
    }
}

/// A new event from its draft. `base` is the vault's base currency.
pub fn plan_create_event(
    doc: &VaultDoc,
    base: &str,
    draft: SharedEventDraft,
    now: &str,
) -> Result<SharedEvent> {
    let title = draft.title.trim().to_string();
    if title.is_empty() {
        return Err(Error::InvalidInput("an event needs a title".into()));
    }
    let (starts_on, ends_on) = check_dates(draft.starts_on, draft.ends_on)?;
    let rounding_step = check_step(draft.rounding_step)?;
    let category_id = check_category(doc, draft.category_id.as_deref())?;
    let mut participants: Vec<Participant> = Vec::new();
    for person in &draft.participants {
        let built = build_participant(doc, person, &participants)?;
        participants.push(built);
    }
    let organizer = check_organizer(&draft.organizer, &participants)?;
    Ok(SharedEvent {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        created_at: now.to_string(),
        note: draft.note.trim().to_string(),
        archived: false,
        starts_on,
        ends_on,
        currency: event_currency(base, &draft.currency),
        rounding_step,
        category_id,
        organizer,
        i_participate: draft.i_participate,
        participants,
        items: Vec::new(),
        settlements: Vec::new(),
        adjustments: Vec::new(),
        removed_with_payment: Vec::new(),
    })
}

/// The event's own fields changed. Things the purchases and settlements
/// already depend on stay put: the currency, who organizes, whether I take
/// part (while it would strand my payments).
pub fn plan_update_event(
    doc: &VaultDoc,
    base: &str,
    id: &str,
    draft: SharedEventDraft,
) -> Result<SharedEvent> {
    let title = draft.title.trim().to_string();
    if title.is_empty() {
        return Err(Error::InvalidInput("an event needs a title".into()));
    }
    let (starts_on, ends_on) = check_dates(draft.starts_on, draft.ends_on)?;
    let rounding_step = check_step(draft.rounding_step)?;
    let category_id = check_category(doc, draft.category_id.as_deref())?;
    let currency = event_currency(base, &draft.currency);
    let mut event = find_event(doc, id)?.clone();
    let organizer = if draft.organizer.trim().is_empty() {
        event.organizer.clone()
    } else {
        check_organizer(&draft.organizer, &event.participants)?
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
    Ok(event)
}

/// An event that can be deleted or archived must be there; an event goes to
/// the archive only when settled (or only planned).
pub fn check_archivable(doc: &VaultDoc, id: &str, archived: bool) -> Result<()> {
    let event = find_event(doc, id)?;
    if archived {
        let status = split::status(event, &split::compute(event));
        if status != SharedEventStatus::Closed && status != SharedEventStatus::Planning {
            return Err(Error::InvalidInput(
                "only a settled event can be archived".into(),
            ));
        }
    }
    Ok(())
}

// -- people -------------------------------------------------------------------

/// The event with one more participant.
pub fn plan_add_participant(
    doc: &VaultDoc,
    event_id: &str,
    draft: SharedParticipantDraft,
) -> Result<SharedEvent> {
    let mut event = find_event(doc, event_id)?.clone();
    let participant = build_participant(doc, &draft, &event.participants)?;
    event.participants.push(participant);
    Ok(event)
}

/// Rename a guest, change their details, or link them to a contact (their
/// debts then show in the contact's card).
pub fn plan_update_participant(
    doc: &VaultDoc,
    event_id: &str,
    participant_id: &str,
    draft: SharedParticipantDraft,
) -> Result<SharedEvent> {
    let mut event = find_event(doc, event_id)?.clone();
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
        build_participant(doc, &SharedParticipantDraft { id: None, ..draft }, &others)?;
    rebuilt.id = participant_id.to_string();
    let old_contact = event
        .participants
        .iter()
        .find(|person| person.id == participant_id)
        .and_then(|person| person.contact_id.clone());
    if rebuilt.contact_id != old_contact {
        let has_operation = event.settlements.iter().any(|settlement| {
            settlement.tx_id.is_some()
                && (settlement.participant_id == participant_id
                    || event.organizer == participant_id)
        });
        if has_operation {
            return Err(Error::InvalidInput(
                "the contact cannot change: operations were made with it in this event".into(),
            ));
        }
    }
    if let Some(person) = event
        .participants
        .iter_mut()
        .find(|person| person.id == participant_id)
    {
        *person = rebuilt;
    }
    Ok(event)
}

/// Take someone out of the event; refused once money or a hand-set total
/// mentions them.
pub fn plan_remove_participant(
    doc: &VaultDoc,
    event_id: &str,
    participant_id: &str,
) -> Result<SharedEvent> {
    let mut event = find_event(doc, event_id)?.clone();
    if !event
        .participants
        .iter()
        .any(|person| person.id == participant_id)
    {
        return Err(Error::Missing(format!("participant {participant_id}")));
    }
    if let Some(item) = event
        .items
        .iter()
        .find(|item| split::only_sharer(&event, item, participant_id))
    {
        return Err(Error::InvalidInput(format!(
            "this person is the only one sharing «{}»: change how it is split first",
            item.title
        )));
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
    Ok(event)
}

/// A guest turned into a contact of the directory.
pub struct GuestContact {
    pub contact: Contact,
    /// Whether the contact is new; `false` when another device made it first.
    pub created: bool,
    /// The event with the guest linked to the contact.
    pub event: SharedEvent,
}

/// "Make a contact": a guest becomes a contact of the directory with the
/// details they had, and is linked to it.
pub fn plan_guest_contact(
    doc: &VaultDoc,
    event_id: &str,
    participant_id: &str,
    contact_id: &str,
    now: &str,
) -> Result<GuestContact> {
    let mut event = find_event(doc, event_id)?.clone();
    let guest = event
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
    // One contact per guest, whichever device makes it: the id (given by the
    // scenario, derived from the guest) and the ids of its accounts are not random.
    let mut accounts: Vec<ContactAccount> = Vec::new();
    if !details.phone.is_empty() {
        accounts.push(ContactAccount {
            id: format!("{contact_id}-phone"),
            active: true,
            label: "Телефон".into(),
            kind: DetailKind::Phone,
            value: details.phone.clone(),
            ..ContactAccount::default()
        });
    }
    if !details.messenger.is_empty() {
        accounts.push(ContactAccount {
            id: format!("{contact_id}-messenger"),
            active: true,
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
            .enumerate()
            .map(|(index, account)| ContactAccount {
                id: format!("{contact_id}-req{index}"),
                active: true,
                ..account
            }),
    );
    let known = doc.contacts.iter().find(|known| known.id == contact_id);
    // Another device made it and the link has not arrived yet.
    let (contact, created) = match known {
        Some(known) => (known.clone(), false),
        None => (
            Contact {
                id: contact_id.to_string(),
                name: guest.name.trim().to_string(),
                kind: ContactKind::Person,
                accounts: crate::contacts::clean_accounts(accounts)?,
                note: details.note.trim().to_string(),
                archived: false,
                created_at: now.to_string(),
                group: String::new(),
            },
            true,
        ),
    };
    if let Some(person) = event
        .participants
        .iter_mut()
        .find(|person| person.id == participant_id)
    {
        person.contact_id = Some(contact.id.clone());
        person.details = None;
    }
    Ok(GuestContact {
        contact,
        created,
        event,
    })
}

/// Fix a person's total by hand (the rest is spread over the others), or take
/// the fixed total back with `None`.
pub fn plan_share_override(doc: &VaultDoc, input: SetShareOverrideInput) -> Result<SharedEvent> {
    let mut event = find_event(doc, &input.event_id)?.clone();
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
    let mut adjustments: Vec<ShareOverride> = event
        .adjustments
        .iter()
        .filter(|fix| fix.participant_id != input.participant_id)
        .cloned()
        .collect();
    if let Some(total) = input.total {
        if total.is_negative() {
            return Err(Error::InvalidInput("a total cannot be negative".into()));
        }
        adjustments.push(ShareOverride {
            participant_id: input.participant_id,
            total,
        });
    }
    let spent = split::compute(&SharedEvent {
        adjustments: Vec::new(),
        ..event.clone()
    })
    .spent;
    let fixed: Vec<(String, Amount)> = adjustments
        .iter()
        .map(|fix| (fix.participant_id.clone(), fix.total))
        .collect();
    if !split::overrides_fit(&fixed, &split::sharer_pool(&event), spent) {
        return Err(Error::InvalidInput(
            "the fixed totals must not exceed what was bought, and must add up to it when everybody is fixed".into(),
        ));
    }
    event.adjustments = adjustments;
    Ok(event)
}

// -- items --------------------------------------------------------------------

fn clean_item(doc: &VaultDoc, draft: &SharedItemDraft) -> Result<(String, String, Option<String>)> {
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
        check_category(doc, draft.category_id.as_deref())?,
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

/// The event with one more item.
pub fn plan_add_item(
    doc: &VaultDoc,
    event_id: &str,
    draft: SharedItemDraft,
) -> Result<SharedEvent> {
    let mut event = find_event(doc, event_id)?.clone();
    let (title, unit, category_id) = clean_item(doc, &draft)?;
    check_plan(&draft)?;
    event.items.push(EventItem {
        id: uuid::Uuid::new_v4().to_string(),
        title,
        unit,
        category_id,
        plan: draft.plan,
        purchases: Vec::new(),
        cancelled: draft.cancelled,
        split: ItemSplit::default(),
    });
    Ok(event)
}

/// The event with one item changed.
pub fn plan_update_item(
    doc: &VaultDoc,
    event_id: &str,
    item_id: &str,
    draft: SharedItemDraft,
) -> Result<SharedEvent> {
    let (title, unit, category_id) = clean_item(doc, &draft)?;
    check_plan(&draft)?;
    let mut event = find_event(doc, event_id)?.clone();
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
    Ok(event)
}

/// The event without an item nothing was bought for.
pub fn plan_delete_item(doc: &VaultDoc, event_id: &str, item_id: &str) -> Result<SharedEvent> {
    let mut event = find_event(doc, event_id)?.clone();
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
    Ok(event)
}

/// The event with the new split of an item: who shares it and how.
pub fn plan_item_split(doc: &VaultDoc, input: SetSharedItemSplitInput) -> Result<SharedEvent> {
    let mut event = find_event(doc, &input.event_id)?.clone();
    let known = |who: &String, event: &SharedEvent| {
        (*who == ME && event.i_participate)
            || event.participants.iter().any(|person| &person.id == who)
    };
    let split = match input.split {
        ItemSplit::Equal { among } => {
            if let Some(unknown) = among.iter().find(|who| !known(who, &event)) {
                return Err(Error::Missing(format!("participant {unknown}")));
            }
            ItemSplit::Equal { among }
        }
        ItemSplit::ByUnits { units } => {
            if let Some(unknown) = units.keys().find(|who| !known(who, &event)) {
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
    Ok(event)
}
