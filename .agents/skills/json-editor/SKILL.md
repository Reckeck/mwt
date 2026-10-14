---
name: json-editor
description: >
  Редактор vault в формате JSON (Настройки → Просмотр JSON, dev-only):
  CodeMirror, схема документа из Rust, автодополнение по JSON-пути, Ctrl+S,
  .bak перед записью, $EDITOR. Использовать при правках crates/app/src/commands/settings.rs
  (vault_schema, dev_dump_vault, dev_apply_vault_json, dev_open_vault_json_in_editor,
  dev_reload_vault_json_from_editor), crates/core/src/model/vault.rs (json_schema),
  src/shared/lib/editor.ts, src/shared/lib/json.ts, src/shared/lib/json-schema-reader.ts,
  src/widgets/vault-json-editor/VaultJsonEditor.vue, а также на задачах «редактор vault»,
  «JSON vault», «правка документа», «подсказки полей», «Ctrl+S», «схема vault»,
  «autocomplete JSON», «VaultFile::backup», «отклонён документ», «черновик»,
  «dev_apply_vault_json», «vault_schema», «Открыть в редакторе».
---

# Редактор vault в формате JSON

Документ vault показывается и правится как есть: расшифрованный `VaultDoc` в
CodeMirror. Экран живёт в `src/widgets/vault-json-editor/VaultJsonEditor.vue`,
схема приходит из Rust по IPC. Полное описание — `docs/json-editor.md`;
здесь только то, что нужно, чтобы не сломать.

## Три правила, которые легко нарушить

1. **Черновик не пишет в vault.** Только Ctrl+S (`dev_apply_vault_json`).
   Ввод текста лишь перепроверяет черновик с дебаунсом и двигает статус-бейдж.
2. **Неразбирающийся черновик не сохраняется.** И на фронте (`parseJson` до
   вызова), и в ядре (`storage::decode` до `replace_doc`) — два независимых
   отказа. Не обходи это «на всякий случай».
3. **Форма документа живёт только в Rust.** Фронт не знает полей vault —
   он спрашивает `vault_schema` и строит подсказки из ответа. Добавил поле в
   `VaultDoc` — подсказки обновились сами.

## Слои

| Слой | Файл |
|---|---|
| Схема (`schemars` по `VaultDoc`) | `crates/core/src/model/vault.rs` → `json_schema()` |
| IPC-хендлеры | `crates/app/src/commands/settings.rs` |
| Бэкап перед записью | `VaultFile::backup` (вызывает только `dev_apply_vault_json`) |
| Разбор `$ref` / `anyOf` / `oneOf` | `src/shared/lib/json-schema-reader.ts` |
| Расширения CodeMirror, тема, autocomplete | `src/shared/lib/editor.ts` |
| `parseJson` / `normalizeJson` | `src/shared/lib/json.ts` |
| Компонент + статус-бейдж | `src/widgets/vault-json-editor/VaultJsonEditor.vue` |

## Что менять и где

**Новое поле vault** → `crates/core/src/model/vault.rs`. Больше ничего: схема
и подсказки подхватят сами. Если меняешь только DTO — `mise run gen-types` и
`mise run test`.

**Новая подсказка в списке** → `src/shared/lib/json-schema-reader.ts`. Схема от
`schemars` приходит вся, включая `$defs`; `$ref`, `Option<T>` как
`anyOf` с `null`, `enum` и как плоский список, и как `oneOf` из `const` —
все четыре варианта уже разобраны, дублировать не надо.

**Цвета редактора** → `HighlightStyle` в `src/shared/lib/editor.ts` на CSS-переменных
темы. Не `.cm-*` правилами в CSS: без зарегистрированного хайтлайтера
CodeMirror генерирует классы, на которые CSS не попадёт, и документ рендерится
без цвета.

**Путь для подсказок** → берётся из `syntaxTree()` (Lezer), не из
собственного парсинга строк. Не переписывай на сканер: он верен по построению.

**Баланс «сохранено / не сохранено»** → сравнение через `normalizeJson`, то
есть канонический `JSON.stringify(JSON.parse(...))`. Пробелы и порядок ключей
не должны считаться правкой: backend всегда возвращает своё форматирование.

## Ошибки

Синтаксис — инлайн под редактором (связан с кареткой, должен читаться рядом).
Всё остальное — `toast` из `vue-sonner`: успешная запись, отказ backend,
ошибки загрузки схемы и документа, работа с `$EDITOR`. Одно событие — либо
toast, либо инлайн, не оба.

Коды ошибок backend (`DESERIALIZE`, `SERIALIZE`, `INVALID_INPUT`, …) приходят
префиксом к сообщению; `errorText()` ищет `errors.<CODE>` в локалях и
откатывается на сырое сообщение.

## Грабли

| Симптом | Причина |
|---|---|
| Документ без подсветки | хайтлайтер задан как `.cm-*` в CSS вместо `HighlightStyle` |
| Статус вечно «есть несохранённые правки» | сравнение идёт по сырому тексту, а не через `normalizeJson` |
| Бейдж «синхронизировано» сразу после Ctrl+S | база обновлена из отправленного текста, а не из ответа backend |
| Каретка прыгает после сохранения | не сохраняешь `hasFocus` и `selection.main.head` до `invalidateQueries()` — диалог перерисовывается и забирает фокус |
| `invoke` → «command … not found» | команда не в `generate_handler!` |
