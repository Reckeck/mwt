# Interface copy

Texts live in `src/shared/config/i18n/locales/{ru,en}.json`, flat keys, the key
sets match (`check-onboarding`). Tour and "What's new" texts — the
`onboarding` skill.

## Tone

- From the point of view of the user and their action: "Enter the actual
  balance — the difference will be recorded as an adjustment", not "The
  component allows…".
- One action is called the same thing along the whole path: the button
  "Archive" -> the toast "Wallet archived" -> the filter "Archive".
- A button is a verb with an object if it is not obvious: "Add transaction",
  "Save", not "OK".
- An error says what did not happen and how to fix it: "Could not save the
  budget" + "The limit must be greater than zero".
- An empty state says what will be here and where to start.
- No internal terms: not `store`, `binding`, `flow_id`, "sub-account of type
  market".

## Writing conventions

- Russian: "ё" is written; guillemets «», dash "—", ellipsis "…".
- English: sentence case ("Add wallet", not "Add Wallet"), “typographic”
  quotes.
- A menu item followed by one more step (a dialog) ends with "…":
  "Rename…".
- Quantities are digits: "3 transactions"; pluralization goes through the
  vue-i18n plural form (`t('key', { count })`).
- Numbers, dates, currencies — only through `format.ts` (not in the key text).

## Glossary

The terms are given in English with the Russian UI word in guillemets (the
value in `ru.json`), because the Russian wording is part of the rule.

| Say | Do not say |
|---|---|
| wallet (bank, exchange, cash, crypto) — «кошелёк» | store, «хранилище» |
| sub-account — «субсчёт» | child store |
| transaction (operation) — «операция» | «транзакция» (in the interface) |
| asset — «актив» | token (except in a crypto context) |
| vault — the data file, «хранилище (vault)» | database, «база» |
| workflow — «схема (workflow)» | graph, «граф», «воркфлоу» |
| archive / to archive — «архив / архивировать» | hide, deactivate, «скрыть», «деактивировать» |
