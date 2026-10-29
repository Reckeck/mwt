// Rules for sums drawn as text (docs/ui-money.md): a sum is a `<Money>` so that hovering it shows
// the other currencies and a click opens its card. Text built by `formatMoney` in a template, or
// returned by a `lib/` or `model/` function of a slice, has neither.
//
// Allowed on purpose, with the reason on the line above (or the same line):
//   // money: plain — <reason>        before a helper's declaration, or in a .ts file
//   <!-- money: plain — <reason> -->  before the element
import { templateOf } from './lib.mjs'

const LABEL = /money: plain\s*[—–-]+\s*\S/

/** Files where a sum is text by design: the currency slice itself, charts, Ctrl K, the browser mock. */
const WHITELIST = [
  /^src\/entities\/currency\//,
  /^src\/shared\/lib\/chart-theme\.ts$/,
  /chart[^/]*\.ts$/,
  /\/command-palette\//,
  /^src\/shared\/api\/mock\//,
  /\.test\.ts$/,
]
const whitelisted = (file) => WHITELIST.some((pattern) => pattern.test(file))

/** Elements whose text is a menu item, an option or a hint: a sum there stays text. */
const TEXT_TAGS = /^(?:option|title|SelectItem|CommandItem|Toast\w*|Tooltip\w*|Money|DropdownMenuItem|ContextMenuItem)$/
const VOID_TAGS = new Set(['br', 'hr', 'img', 'input', 'link', 'meta'])
const TAG = /<(\/?)([A-Za-z][\w.-]*)((?:"[^"]*"|'[^']*'|[^>"'])*)>/g

const lineStart = (text, index) => text.lastIndexOf('\n', index - 1) + 1
const labelled = (source, index) => {
  const from = lineStart(source, index)
  const before = source.slice(Math.max(0, source.lastIndexOf('\n', from - 2) - 200), index)
  const lines = before.split('\n').slice(-3)
  return lines.some((line) => LABEL.test(line)) || LABEL.test(source.slice(from, source.indexOf('\n', index)))
}

/** Names of the script's helpers that end in `formatMoney(` / `moneyParts(` (directly or through another helper). */
function moneyHelpers(script, source, offset) {
  const decls = [...script.matchAll(/^(?:export\s+)?(?:const|let|function)\s+(\w+)/gm)]
  const chunks = decls.map((match, at) => {
    const end = decls[at + 1]?.index ?? script.length
    return { name: match[1], body: script.slice(match.index, end), index: match.index + offset }
  })
  const helpers = new Set()
  const direct = /\b(?:formatMoney|moneyParts)\s*\(|\buseMoney\s*\(/
  for (let round = 0; round < 3; round++) {
    for (const chunk of chunks) {
      if (helpers.has(chunk.name) || labelled(source, chunk.index)) continue
      const calls = [...helpers].some((name) => new RegExp(`(?<![.\\w$])${name}\\s*\\(`).test(chunk.body))
      if (direct.test(chunk.body) || calls) helpers.add(chunk.name)
    }
  }
  // `const { money } = useMoney()`: the destructured names are helpers.
  for (const match of script.matchAll(/const\s*\{([^}]*)\}\s*=\s*useMoney\s*\(/g)) {
    for (const name of match[1].split(',')) helpers.add(name.split(':').pop().trim())
  }
  // A number without its mark next to a `<Money>` (`money(x).replace(…)`, `saved / goal`): the
  // mark is the sum beside it, which is a Money. Not a text sum.
  return helpers
}

/** The `{{ … }}` of a chunk with their code; braces inside (an object argument) are balanced. */
function mustaches(chunk) {
  const found = []
  for (let open = chunk.indexOf('{{'); open !== -1; open = chunk.indexOf('{{', open + 2)) {
    let depth = 0
    for (let at = open + 2; at < chunk.length; at++) {
      if (chunk[at] === '{') depth++
      else if (chunk[at] === '}') {
        if (depth === 0 && chunk[at + 1] === '}') {
          found.push({ code: chunk.slice(open + 2, at), index: open })
          open = at
          break
        }
        depth--
      }
    }
  }
  return found
}

function textSums(text, file, source) {
  const parts = templateOf(text)
  if (!parts) return []
  const script = text.slice(0, parts.start)
  const helpers = moneyHelpers(script, source, 0)
  const names = ['formatMoney', 'moneyParts', ...helpers]
  const uses = new RegExp(`(?<![.\\w$'"\`])(?:${names.join('|')})(?![\\w$:'"\`])`)
  const hasMoney = /<Money\b/.test(parts.body)
  // A helper that only trims a sum to its number, beside a <Money> for the pair's other half.
  const bareOnly = (code) => hasMoney && /\.replace\(/.test(code)
  const hits = []

  // Walk the tags once: which element each mustache and attribute sits in.
  const stack = []
  let last = 0
  const scanText = (from, to) => {
    const chunk = parts.body.slice(from, to)
    for (const mustache of mustaches(chunk)) {
      if (stack.some((tag) => TEXT_TAGS.test(tag)) || !uses.test(mustache.code) || bareOnly(mustache.code)) continue
      hits.push({ index: parts.start + from + mustache.index })
    }
  }
  for (const match of parts.body.matchAll(TAG)) {
    scanText(last, match.index)
    last = match.index + match[0].length
    const [, closing, name, attrs] = match
    if (closing) {
      const at = stack.lastIndexOf(name)
      if (at !== -1) stack.length = at
      continue
    }
    if (!stack.some((tag) => TEXT_TAGS.test(tag)) && !TEXT_TAGS.test(name)) {
      for (const attr of attrs.matchAll(/\s:(?:label|text|value|hint|description|subtitle|line|caption|message|summary|detail)="([^"]*)"/g)) {
        const code = attr[1]
        if (!uses.test(code) || bareOnly(code)) continue
        // A StatCard shows its sum as a <Money> in the slot; the attribute is its plain fallback.
        const inner = parts.body.slice(last, last + 800).split(`</${name}>`)[0]
        if (name === 'StatCard' && /<Money\b/.test(inner)) continue
        hits.push({ index: parts.start + match.index + attr.index })
      }
    }
    // A list built from helpers that print sums (`v-for="row in rows"`): its rows carry text sums.
    const listed = attrs.match(/\sv-for="[^"]*\sin\s+(\w+)\s*"/)
    if (listed && helpers.has(listed[1]) && !stack.some((tag) => TEXT_TAGS.test(tag))) {
      hits.push({ index: parts.start + match.index + listed.index })
    }
    if (!attrs.endsWith('/') && !VOID_TAGS.has(name)) stack.push(name)
  }
  return hits.filter((hit) => !labelled(source, hit.index))
}

export const MONEY_RULES = [
  {
    id: 'money-text',
    vueOnly: true,
    skip: whitelisted,
    find: textSums,
    say: 'a sum as text in a template (formatMoney or a helper over it) — use <Money :amount :currency [:at]>, or mark why with `money: plain — <reason>`',
  },
  {
    id: 'money-lib',
    vueOnly: false,
    skip: (file) =>
      file.endsWith('.vue') || whitelisted(file) || !/^src\/(?:pages|widgets|features|entities)\/.*\/[^/]+\.ts$/.test(file),
    find: (text, _file, source) =>
      [...text.matchAll(/(?<![.\w$])formatMoney\s*\(/g)].filter(
        (match) => !labelled(source, match.index) && !LABEL.test(source.split('\n').slice(0, 3).join('\n')),
      ),
    say: 'formatMoney in a slice’s lib/model returns a string a template prints as is — return { amount, currency, at } and draw <Money>, or mark `money: plain — <reason>`',
  },
  {
    // `Can spend today` had `:convert="false"`: the figure did not open the currency card.
    id: 'money-home-figure',
    vueOnly: true,
    skip: (file) => !/^src\/pages\/dashboard\/ui\/home-[^/]+\//.test(file),
    find: (text, _file, source) =>
      [...text.matchAll(/<Money\b[^>]*:convert="false"/g)].filter((match) => !labelled(source, match.index)),
    say: 'a sum on Home with :convert="false" does not open the currency card — drop it, or mark why with `money: plain — <reason>`',
  },
]
