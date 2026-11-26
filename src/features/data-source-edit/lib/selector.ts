/**
 * A short CSS selector for an element the user clicked in the page preview:
 * an id if it is stable, then stable classes, then `nth-of-type` steps up to
 * the nearest anchor. Generated ids and hashed classes (`css-1x2y3z`,
 * `jsx-123`) change between visits, so they are skipped.
 */

const UNSTABLE = /\d{3,}|^(css|jsx|sc|svelte|emotion|chakra)-|^_|[A-Z].*[0-9]|--/

function stableId(element: Element): string | null {
  const id = element.id
  return id && !UNSTABLE.test(id) && /^[A-Za-z][\w-]*$/.test(id) ? id : null
}

function stableClasses(element: Element): string[] {
  return [...element.classList].filter((name) => !UNSTABLE.test(name) && /^[A-Za-z][\w-]*$/.test(name)).slice(0, 2)
}

/** One step: `tag.class` when it is unique among its siblings, otherwise `tag:nth-of-type(n)`. */
function step(element: Element): string {
  const tag = element.tagName.toLowerCase()
  const parent = element.parentElement
  const classes = stableClasses(element)
  const withClasses = classes.length ? `${tag}.${classes.join('.')}` : tag
  if (!parent) return withClasses
  const siblings = [...parent.children].filter((child) => child.tagName === element.tagName)
  if (siblings.length === 1) return withClasses
  if (classes.length && parent.querySelectorAll(`:scope > ${withClasses}`).length === 1) return withClasses
  return `${tag}:nth-of-type(${siblings.indexOf(element) + 1})`
}

/** The selector of `element` inside `doc`, checked to match it first. */
export function buildSelector(element: Element, doc: Document): string {
  const parts: string[] = []
  let current: Element | null = element
  while (current && current !== doc.documentElement && current !== doc.body) {
    const id = stableId(current)
    if (id) {
      parts.unshift(`#${id}`)
      break
    }
    parts.unshift(step(current))
    const candidate = parts.join(' > ')
    if (doc.querySelectorAll(candidate).length === 1 && doc.querySelector(candidate) === element) return candidate
    current = current.parentElement
  }
  return parts.join(' > ')
}

/** Visible text of an element with whitespace collapsed, as the backend reads it. */
export function textOf(element: Element): string {
  return (element.textContent ?? '').replace(/\s+/g, ' ').trim()
}

/** When the click is inside a table: the table's selector and the header of the clicked column. */
export function tableContext(element: Element, doc: Document): { selector: string; header: string } | null {
  const cell = element.closest('td, th')
  const table = element.closest('table')
  if (!cell || !table) return null
  const row = cell.parentElement
  const index = row ? [...row.children].indexOf(cell) : -1
  const headerRow = table.querySelector('tr')
  const header = headerRow?.children[index]
  return { selector: buildSelector(table, doc), header: header ? textOf(header) : '' }
}

/** The text right before the clicked element in its parent: a label for `text_hint`. */
export function hintOf(element: Element): string {
  const parent = element.parentElement
  if (!parent) return ''
  const own = textOf(element)
  const whole = textOf(parent)
  const at = own ? whole.indexOf(own) : -1
  const before = at > 0 ? whole.slice(0, at) : ''
  return before.trim().split(' ').slice(-4).join(' ')
}
