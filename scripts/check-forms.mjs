// The forms gate (AGENTS.md rule 13, skill `ui-design-system` → «Формы»): a
// form is never saved by guessing why a button is grey. Its save button stays
// enabled; pressing it with a missing or wrong field shows the message under
// that field (`useFormValidation` + `FormField`).
//
// Static: reads the `.vue` sources, runs nothing. Exit code 1 on an error.
//
// Errors:
// - a submit button (`type="submit"` or `form="…"`) whose `:disabled` waits for
//   something other than the save in flight;
// - `@submit` that checks fields itself (`valid && save()`), which saves
//   nothing and says nothing.
//
// A button that is off for a reason the user cannot fix in the form (the
// backend refuses the plan) is allowed with a name from ALLOWED; it must say
// why next to it.
//
// Rule 14 — an account, an asset or a currency is chosen with the ready
// component, never a hand-made list:
// - `<option v-for>` over wallets (`….store.id`) → `StoreNodeSelect`;
// - `<option v-for>` over assets → `AssetSelect` (or `AssetInput` to search);
// - `<option v-for>` over currency codes, or an `<Input>` typed into a
//   currency → `CurrencyPicker`;
// - `<ToggleGroupItem v-for>` over currency codes → `ChartCurrencySwitch` or
//   `CurrencyPicker` (they put the favourites first);
// - an `<Input>` typed into a symbol, pair, ticker or contract → `PairInput`
//   or `AssetInput`; a field that is not an asset says so with
//   `data-picker-ok`.
// - a `<SelectItem>` / `<option>` / `<ComboboxItem>` per contact (`v-for="contact in …"`) →
//   `ContactPicker` (search, «Создать «Петя»» on the spot); a pick-only filter says `no-create`.
// The components themselves (PICKERS) are where such lists live.
import { readdirSync, readFileSync } from 'node:fs'
import { join, relative } from 'node:path'

const root = new URL('..', import.meta.url).pathname
const errors = []

/** What a submit button may wait for: a save in flight, or a refusal shown with its reason. */
const ALLOWED = /^(?:[a-zA-Z]+\.)*(?:isPending(?:\.value)?|pending|busy|refused|loading)$/

function walk(dir, out = []) {
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const path = join(dir, entry.name)
    if (entry.isDirectory()) walk(path, out)
    else if (entry.name.endsWith('.vue')) out.push(path)
  }
  return out
}

const lineOf = (text, index) => text.slice(0, index).split('\n').length

/** The ready pickers: the only places a list of wallets, assets or currencies is drawn by hand. */
const PICKERS = /(?:ContactPicker|StoreNodeSelect|AssetSelect|AssetInput|CurrencyPicker|AccountRefPicker|ChartCurrencySwitch|PairInput)\.vue$/

for (const file of walk(join(root, 'src'))) {
  const text = readFileSync(file, 'utf8')
  const where = (index) => `${relative(root, file)}:${lineOf(text, index)}`

  // Every <Button …> / <button …> opening tag, attributes across lines included.
  for (const match of text.matchAll(/<(Button|button)\b([^>]*?)>/gs)) {
    const attrs = match[2]
    const isSubmit = /\btype="submit"/.test(attrs) || /(?<![:\w-])form="[^"]+"/.test(attrs)
    if (!isSubmit) continue
    const disabled = attrs.match(/:disabled="([^"]*)"/)
    if (!disabled) continue
    const parts = disabled[1].split(/\|\||&&/).map((part) => part.trim().replace(/^\((.*)\)$/, '$1'))
    const blocked = parts.filter((part) => !ALLOWED.test(part.replace(/^form\?\./, '')))
    if (blocked.length) {
      errors.push(
        `${where(match.index)}: submit button is disabled by «${blocked.join(' || ')}» — keep it enabled and check the fields with useFormValidation`,
      )
    }
  }

  if (!PICKERS.test(file)) {
    for (const match of text.matchAll(/<option\b[^>]*v-for="\s*\(?\s*(\w+)[^"]*\bin\s+([\w.]+)"([^>]*)>/gs)) {
      const [, item, list, rest] = match
      const attrs = match[0]
      if (new RegExp(`\\b${item}\\.store\\.id\\b`).test(attrs)) {
        errors.push(`${where(match.index)}: a wallet list by hand («${list}») — use StoreNodeSelect (rule 14)`)
      } else if (/^(?:asset|assets?|held\w*)$/i.test(item) || /assets$/i.test(list)) {
        errors.push(`${where(match.index)}: an asset list by hand («${list}») — use AssetSelect or AssetInput (rule 14)`)
      } else if (/^(?:code|currency)$/i.test(item) && /(?:codes|currencies)$/i.test(list) && !rest.includes('data-picker-ok')) {
        errors.push(`${where(match.index)}: a currency list by hand («${list}») — use CurrencyPicker (rule 14)`)
      }
    }
    // Quoted attributes may hold `>` (`(entry) => …`), so the tag is read quote by quote.
    for (const match of text.matchAll(/<(?:SelectItem|option|ComboboxItem)\b(?:[^>"]|"[^"]*")*/g)) {
      const loop = match[0].match(/v-for="\s*\(?\s*(\w+)[^"]*?\bin\s+([^"]+)"/)
      if (loop && (/contact/i.test(loop[1]) || /\bcontacts?\b/i.test(loop[2]))) {
        errors.push(`${where(match.index)}: a contact list by hand («${loop[2].trim()}») — use ContactPicker (rule 14)`)
      }
    }
    for (const match of text.matchAll(/<ToggleGroupItem\b[^>]*v-for="\s*\(?\s*(code|currency)\b[^"]*"/gs)) {
      errors.push(`${where(match.index)}: currencies as hand-made toggles — use ChartCurrencySwitch or CurrencyPicker (rule 14)`)
    }
    for (const match of text.matchAll(/<Input\b([^>]*?)>/gs)) {
      if (/v-model="[^"]*currency[^"]*"/i.test(match[1])) {
        errors.push(`${where(match.index)}: a currency typed into <Input> — use CurrencyPicker (rule 14)`)
      } else if (/v-model="[^"]*(?:symbol|pair|ticker|contract)[^"]*"/i.test(match[1]) && !match[1].includes('data-picker-ok')) {
        errors.push(`${where(match.index)}: a pair or ticker typed into <Input> — use PairInput or AssetInput (rule 14)`)
      }
    }
  }

  for (const match of text.matchAll(/@submit\.prevent="([^"]*)"/g)) {
    if (/&&/.test(match[1])) {
      errors.push(`${where(match.index)}: @submit checks fields by hand («${match[1]}») — use form.submit(save, $event)`)
    }
  }
}

for (const error of errors) console.error(`error: ${error}`)
console.log(`check-forms: ${errors.length} errors`)
process.exit(errors.length ? 1 : 0)
