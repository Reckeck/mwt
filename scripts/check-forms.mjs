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

  for (const match of text.matchAll(/@submit\.prevent="([^"]*)"/g)) {
    if (/&&/.test(match[1])) {
      errors.push(`${where(match.index)}: @submit checks fields by hand («${match[1]}») — use form.submit(save, $event)`)
    }
  }
}

for (const error of errors) console.error(`error: ${error}`)
console.log(`check-forms: ${errors.length} errors`)
process.exit(errors.length ? 1 : 0)
