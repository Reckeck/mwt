// Rules for behaviour that makes the interface lag, flicker or move by itself
// (AGENTS.md rule 17, references/stable-layout.md).
import { atFile, callBody, openTags, templateOf } from './lib.mjs'

/** Files that own the backend event plumbing: the only places `onEvent(` may be called. */
const EVENT_PLUMBING = /^src\/shared\/api\/(?:ipc|use-backend-event|mock-bus)\.ts$|^src\/shared\/api\/mock\//

const CHEAP_CALL = /^(?:String|Number|Boolean|t|\$t)\(/

/** Names set to true by a hover/focus handler, inline (`x = true`) or inside a named handler. */
function hoverArmed(text) {
  const names = new Set()
  for (const match of text.matchAll(/@(?:pointerenter|mouseenter|mouseover|focusin)="([^"]+)"/g)) {
    const handler = match[1].trim()
    for (const set of handler.matchAll(/\b([A-Za-z_]\w*)\s*=\s*true\b/g)) names.add(set[1])
    const fn = handler.match(/^([A-Za-z_]\w*)(?:\(\))?$/)
    if (!fn) continue
    const body = text.match(new RegExp(`function\\s+${fn[1]}\\s*\\([^)]*\\)\\s*\\{([\\s\\S]*?)\\n\\}`))
    for (const set of body?.[1].matchAll(/\b([A-Za-z_]\w*)\.value\s*=\s*true\b/g) ?? []) names.add(set[1])
  }
  return names
}

export const BEHAVIOUR_RULES = [
  {
    id: 'raw-listener',
    find: (text, file) => {
      const hits = []
      if (!EVENT_PLUMBING.test(file)) hits.push(...text.matchAll(/\bonEvent\s*(?:<[^>]*>)?\(/g))
      if (!/removeEventListener|useEventListener/.test(text)) hits.push(...text.matchAll(/\baddEventListener\(/g))
      if (!/clearInterval/.test(text)) hits.push(...text.matchAll(/(?<![.\w])setInterval\(/g))
      // A kept timer handle must be cleared somewhere (a fire-and-forget `setTimeout(fn)` is fine).
      if (!/clearTimeout/.test(text)) hits.push(...text.matchAll(/=\s*(?:window\.)?setTimeout\(/g))
      return hits
    },
    say: 'a listener or timer without cleanup — backend events via useBackendEvent, DOM via useEventListener, timers cleared in onScopeDispose',
  },
  {
    id: 'hover-mount',
    vueOnly: true,
    find: (text) => {
      const names = hoverArmed(text)
      if (!names.size) return []
      return [...text.matchAll(/\bv-(?:if|else-if|show)="([^"]*)"/g)].filter((match) =>
        [...names].some((name) => new RegExp(`\\b${name}\\b`).test(match[1])),
      )
    },
    say: 'hover or focus mounts content (v-if on a flag set in @pointerenter) — reserve its space or keep it mounted',
  },
  {
    id: 'control-swap',
    vueOnly: true,
    find: (text) => [
      // One control replaced by another: width jumps and the focused element is destroyed.
      ...text.matchAll(/<\/(?:Button|ConfirmButton)>\s*(?:<!--[\s\S]*?-->\s*)*<(?:Button|ConfirmButton)\b(?=[^>]*\bv-else)/g),
      // A spinner added in front of the label widens the button while it waits.
      ...text.matchAll(/<Spinner\b[^>]*\bv-if="[^"]*"[^>]*\/>(?!\s*<[A-Za-z][\w-]*\b[^>]*\bv-else)/g),
    ],
    say: 'a control swapped or widened by state — keep one element with a fixed width; swap the icon, not the button',
  },
  {
    id: 'query-keep-data',
    // A key that names another entity (contact, wallet, operation) must not show the last
    // one's data under the new title: say so inside the call, `// no placeholder: <why>`.
    find: (text, _file, source) =>
      [...text.matchAll(/\buse(?:Infinite)?Query\s*(?:<[^>]*>)?\s*\(/g)].filter((match) => {
        const open = match.index + match[0].length - 1
        const body = callBody(text, open)
        const original = source.slice(open, open + body.length)
        return (
          /\bqueryKey\s*:\s*computed\(/.test(body) &&
          !/\bplaceholderData\b/.test(body) &&
          !/no placeholder: \S/.test(original)
        )
      }),
    say: 'a query with a changing key and no placeholderData — the content blanks on every change (keepPreviousData, or `// no placeholder: <why>` for another entity)',
  },
  {
    id: 'invalidate-all',
    find: (text) => [...text.matchAll(/\binvalidateQueries\(\s*\)/g)],
    say: 'invalidateQueries() with no key refetches every screen — name the keys (or check-ui-ignore with the reason)',
  },
  {
    id: 'intl-cache',
    find: (text) => [...text.matchAll(/\bnew Intl\.\w+\(|\.toLocale(?:Date|Time)?String\(/g)],
    say: 'a formatter built per call — use shared/lib/format (cached Intl formatters)',
  },
  {
    id: 'row-repeat-call',
    vueOnly: true,
    find: (text) => {
      const template = templateOf(text)
      if (!template) return []
      const hits = []
      for (const loop of template.body.matchAll(/\bv-for="\(?\s*([A-Za-z_]\w*)/g)) {
        const rest = template.body.slice(loop.index, loop.index + 4000)
        const calls = new Map()
        for (const call of rest.matchAll(new RegExp(`\\b[A-Za-z_][\\w.]*\\(\\s*${loop[1]}\\b[^()]*\\)`, 'g'))) {
          // Conversions and translations are cheap; the rule is about real work per row.
          if (CHEAP_CALL.test(call[0])) continue
          calls.set(call[0], (calls.get(call[0]) ?? 0) + 1)
        }
        if ([...calls.values()].some((count) => count >= 3)) hits.push(loop)
      }
      return atFile(hits, template.start)
    },
    say: 'the same call repeated per row in a template — compute a row model (computed) or a row component',
  },
  {
    id: 'router-key',
    vueOnly: true,
    find: (text) => [...openTags(text, 'RouterView'), ...openTags(text, 'router-view')].filter((match) => /:key=/.test(match[0])),
    say: 'a keyed RouterView remounts the whole page on every path change — key only what needs it, inside the page',
  },
  {
    id: 'inline-shared-query',
    skip: (file) => file.startsWith('src/entities/') || file.startsWith('src/shared/api/'),
    find: (text) => [...text.matchAll(/\bcall\s*(?:<[^>]*>)?\(\s*'(?:get_store_tree|list_contacts|list_categories)'/g)],
    say: 'a shared list fetched inline — use useStoreTree / useContacts / useCategories from shared/api/queries',
  },
]
