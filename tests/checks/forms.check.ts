// The forms gate (AGENTS.md rules 13 and 14, skill `ui-design-system` → «Формы»): a form is never
// saved by guessing why a button is grey. Its save button stays enabled; pressing it with a missing
// or wrong field shows the message under that field (`useFormValidation` + `FormField`).
// Static: reads the `.vue` sources, runs nothing.
//
// - a submit button (`type="submit"` or `form="…"`) whose `:disabled` waits for something other
//   than the save in flight; `@submit` that checks fields itself (`valid && save()`);
// - rule 14: an account, an asset or a currency is chosen with the ready component, never a
//   hand-made list (`<option v-for>` over wallets / assets / currency codes → `StoreNodeSelect`,
//   `AssetSelect`/`AssetInput`, `CurrencyPicker`; currencies as `<ToggleGroupItem v-for>` →
//   `ChartCurrencySwitch`/`CurrencyPicker`; an `<Input>` typed into a currency, symbol, pair,
//   ticker or contract → `CurrencyPicker`/`PairInput`/`AssetInput`; a field that is not an asset
//   says so with `data-picker-ok`). The components themselves (PICKERS) are where such lists live.
//
// A button that is off for a reason the user cannot fix in the form (the backend refuses the plan)
// is allowed with a name from ALLOWED; it must say why next to it.
import { readFileSync } from 'node:fs'
import { join, relative } from 'node:path'

import { describe, it } from 'vitest'

import { expectNone, ROOT } from './lib/report'
import { lineOf, walk } from './lib/walk'

/** What a submit button may wait for: a save in flight, or a refusal shown with its reason. */
const ALLOWED = /^(?:[a-zA-Z]+\.)*(?:isPending(?:\.value)?|pending|busy|refused|loading)$/

/** The ready pickers: the only places a list of wallets, assets or currencies is drawn by hand. */
const PICKERS =
  /(?:StoreNodeSelect|AssetSelect|AssetInput|CurrencyPicker|AccountRefPicker|ChartCurrencySwitch|PairInput)\.vue$/

type Finding = { rule: 'rule-13-submit' | 'rule-13-submit-handler' | 'rule-14-pickers'; text: string }

function scan(): Finding[] {
  const found: Finding[] = []
  for (const file of walk(join(ROOT, 'src'), '.vue')) {
    const text = readFileSync(file, 'utf8')
    const where = (index: number) => `${relative(ROOT, file)}:${lineOf(text, index)}`
    const add = (rule: Finding['rule'], index: number, message: string) =>
      found.push({ rule, text: `${where(index)}: ${message}` })

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
        add(
          'rule-13-submit',
          match.index,
          `submit button is disabled by «${blocked.join(' || ')}» — keep it enabled and check the fields with useFormValidation`,
        )
      }
    }

    if (!PICKERS.test(file)) {
      for (const match of text.matchAll(/<option\b[^>]*v-for="\s*\(?\s*(\w+)[^"]*\bin\s+([\w.]+)"([^>]*)>/gs)) {
        const [whole, item, list, rest] = match
        if (new RegExp(`\\b${item}\\.store\\.id\\b`).test(whole)) {
          add('rule-14-pickers', match.index, `a wallet list by hand («${list}») — use StoreNodeSelect (rule 14)`)
        } else if (/^(?:asset|assets?|held\w*)$/i.test(item) || /assets$/i.test(list)) {
          add(
            'rule-14-pickers',
            match.index,
            `an asset list by hand («${list}») — use AssetSelect or AssetInput (rule 14)`,
          )
        } else if (
          /^(?:code|currency)$/i.test(item) &&
          /(?:codes|currencies)$/i.test(list) &&
          !rest.includes('data-picker-ok')
        ) {
          add('rule-14-pickers', match.index, `a currency list by hand («${list}») — use CurrencyPicker (rule 14)`)
        }
      }
      for (const match of text.matchAll(/<ToggleGroupItem\b[^>]*v-for="\s*\(?\s*(code|currency)\b[^"]*"/gs)) {
        add(
          'rule-14-pickers',
          match.index,
          'currencies as hand-made toggles — use ChartCurrencySwitch or CurrencyPicker (rule 14)',
        )
      }
      for (const match of text.matchAll(/<Input\b([^>]*?)>/gs)) {
        if (/v-model="[^"]*currency[^"]*"/i.test(match[1])) {
          add('rule-14-pickers', match.index, 'a currency typed into <Input> — use CurrencyPicker (rule 14)')
        } else if (
          /v-model="[^"]*(?:symbol|pair|ticker|contract)[^"]*"/i.test(match[1]) &&
          !match[1].includes('data-picker-ok')
        ) {
          add(
            'rule-14-pickers',
            match.index,
            'a pair or ticker typed into <Input> — use PairInput or AssetInput (rule 14)',
          )
        }
      }
    }

    for (const match of text.matchAll(/@submit\.prevent="([^"]*)"/g)) {
      if (/&&/.test(match[1])) {
        add(
          'rule-13-submit-handler',
          match.index,
          `@submit checks fields by hand («${match[1]}») — use form.submit(save, $event)`,
        )
      }
    }
  }
  return found
}

const findings = scan()
const of = (rule: Finding['rule']) => findings.filter((finding) => finding.rule === rule).map((finding) => finding.text)

describe('check-forms', () => {
  it('rule 13: a submit button is blocked only by a save in flight', () => expectNone(of('rule-13-submit')))
  it('rule 13: @submit does not check fields by hand', () => expectNone(of('rule-13-submit-handler')))
  it('rule 14: accounts, assets and currencies come from the ready pickers', () => expectNone(of('rule-14-pickers')))
})
