/// CodeMirror extensions for the raw vault JSON editor.
///
/// Everything mechanical comes from CodeMirror: syntax highlighting, undo
/// history, bracket matching and auto-close, indentation, folding, search and
/// the completion popup with its own key handling. What is added here is
/// (a) a theme mapped onto the app's CSS variables, so light/dark follow the
/// app for free, and (b) one completion source that maps the cursor's JSON
/// path onto the vault schema.
///
/// The path comes from CodeMirror's own Lezer syntax tree, so it is correct by
/// construction — there is no re-implemented string or brace scanning here.

import {
  autocompletion,
  closeBrackets,
  closeBracketsKeymap,
  closeCompletion,
  completionKeymap,
  currentCompletions,
  type Completion,
  type CompletionContext,
  type CompletionResult,
} from '@codemirror/autocomplete'
import { defaultKeymap, history, historyKeymap, indentLess, indentMore } from '@codemirror/commands'
import { json, jsonParseLinter } from '@codemirror/lang-json'
import { tags as t } from '@lezer/highlight'
import {
  bracketMatching,
  foldGutter,
  foldKeymap,
  HighlightStyle,
  indentOnInput,
  syntaxHighlighting,
  syntaxTree,
} from '@codemirror/language'
import { lintKeymap, linter } from '@codemirror/lint'
import { highlightSelectionMatches, searchKeymap } from '@codemirror/search'
import { EditorState, type Extension } from '@codemirror/state'
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
  rectangularSelection,
} from '@codemirror/view'

import { descriptionOf, type JsonSchemaReader, type JsonSchema } from './json-schema-reader'

/** Height the component hands the editor; the view fills its parent. */
export const EDITOR_HEIGHT = '60dvh'

/**
 * Token colours.
 *
 * This has to be a `HighlightStyle` plus `syntaxHighlighting()`, not a set of
 * `.cm-*` rules in the theme: with no highlighter registered, CodeMirror falls
 * back to emitting generated class names that cannot be targeted from CSS, and
 * the document renders uncoloured.
 *
 * Colours are CSS variables, so a `.dark` ancestor switches the editor together
 * with the app and no JS has to know which theme is active.
 */
const vaultHighlight = HighlightStyle.define([
  { tag: t.propertyName, color: 'var(--syntax-key)' },
  { tag: [t.string, t.special(t.string)], color: 'var(--syntax-string)' },
  { tag: [t.number, t.integer, t.float], color: 'var(--syntax-number)' },
  { tag: [t.bool, t.atom], color: 'var(--syntax-literal)' },
  { tag: [t.null, t.comment], color: 'var(--syntax-comment)', fontStyle: 'italic' },
  { tag: [t.keyword, t.operator, t.modifier], color: 'var(--syntax-literal)' },
  { tag: [t.punctuation, t.separator, t.bracket], color: 'var(--muted-foreground)' },
  { tag: [t.definition(t.variableName), t.variableName], color: 'var(--foreground)' },
  { tag: t.invalid, color: 'var(--syntax-invalid)', textDecoration: 'underline wavy' },
])

/**
 * Theme referencing only CSS variables, so a `.dark` ancestor switches the
 * editor together with the app and no JS has to know which theme is active.
 */
const vaultJsonTheme = EditorView.theme({
  '&': {
    height: '100%',
    color: 'var(--foreground)',
    backgroundColor: 'transparent',
    fontSize: '0.75rem',
  },
  '&.cm-focused': { outline: 'none' },
  '.cm-content': { caretColor: 'var(--foreground)', padding: '0.75rem' },
  '.cm-line': { padding: '0' },
  '.cm-cursor, .cm-dropCursor': { borderLeftColor: 'var(--foreground)', borderLeftWidth: '2px' },
  '.cm-selectionBackground, .cm-content ::selection': {
    backgroundColor: 'var(--syntax-selection)',
  },
  '.cm-activeLine': { backgroundColor: 'var(--syntax-active-line)' },
  '.cm-gutters': {
    backgroundColor: 'transparent',
    color: 'var(--muted-foreground)',
    border: 'none',
    borderRight: '1px solid var(--border)',
  },
  '.cm-activeLineGutter': { backgroundColor: 'var(--syntax-active-line)' },
  // Popup and panel chrome, mapped onto the same variables.
  '.cm-tooltip': {
    backgroundColor: 'var(--popover)',
    color: 'var(--popover-foreground)',
    border: '1px solid var(--border)',
    borderRadius: '0.5rem',
    overflow: 'hidden',
  },
  '.cm-tooltip-autocomplete > ul': { fontFamily: 'inherit', fontSize: '0.75rem', maxHeight: '14rem' },
  '.cm-tooltip-autocomplete > ul > li': { padding: '2px 8px' },
  '.cm-tooltip-autocomplete > ul > li[aria-selected]': {
    backgroundColor: 'var(--accent)',
    color: 'var(--accent-foreground)',
  },
  '.cm-completionDetail': { marginLeft: '8px', opacity: '0.55', fontStyle: 'normal' },
  '.cm-completionMatchedText': { textDecoration: 'none', color: 'var(--primary)', fontWeight: '600' },
  '.cm-panels': { backgroundColor: 'var(--popover)', color: 'var(--popover-foreground)' },
  '.cm-panels input, .cm-panels button': {
    backgroundColor: 'var(--card)',
    color: 'var(--card-foreground)',
    border: '1px solid var(--border)',
    borderRadius: '0.25rem',
  },
  '.cm-searchMatch': { backgroundColor: 'var(--syntax-active-line)' },
  '.cm-searchMatch.cm-searchMatch-selected': { backgroundColor: 'var(--syntax-selection)' },
})

/** Strip the quotes Lezer includes in a `PropertyName`. */
function keyText(state: EditorState, from: number, to: number): string {
  return state.sliceDoc(from, to).replace(/^"|"$/g, '')
}

/**
 * Lezer has named this node `Property` and `ObjectProperty` in different
 * versions, so both are accepted rather than pinning us to one.
 */
function isProperty(name: string): boolean {
  return name === 'Property' || name === 'ObjectProperty'
}

/**
 * Keys of the objects enclosing `pos`, outermost first.
 *
 * The innermost `Property` is skipped when the cursor is still inside its name:
 * the caret sits in a key slot there, so that key is not part of the path — but
 * everything *around* it is, which is what puts a bare `"kind"` inside
 * `"stores": [{ … }]` in the store's schema rather than the document root.
 */
function enclosingKeys(state: EditorState, pos: number): string[] {
  const keys: string[] = []
  let innermostSeen = false

  const start = syntaxTree(state).resolveInner(pos, -1)
  for (let node: typeof start | null = start; node; node = node.parent) {
    if (!isProperty(node.name)) continue
    const name = node.getChild('PropertyName')
    if (!name) continue
    if (!innermostSeen) {
      innermostSeen = true
      if (pos <= name.to) continue // caret is inside the key itself
    }
    keys.push(keyText(state, name.from, name.to))
  }

  return keys.reverse()
}

/** Keys already present in the object the cursor sits in. */
function siblingKeys(state: EditorState, pos: number): string[] {
  const start = syntaxTree(state).resolveInner(pos, -1)
  for (let node: typeof start | null = start; node; node = node.parent) {
    if (node.name !== 'Object') continue
    const keys: string[] = []
    const cursor = node.cursor()
    if (cursor.firstChild()) {
      do {
        if (!isProperty(cursor.name)) continue
        const name = cursor.node.getChild('PropertyName')
        if (name) keys.push(keyText(state, name.from, name.to))
      } while (cursor.nextSibling())
    }
    return keys
  }
  return []
}

/** Whether the cursor is inside an existing string value rather than after `:`. */
function insideString(state: EditorState, pos: number): boolean {
  const start = syntaxTree(state).resolveInner(pos, -1)
  for (let node: typeof start | null = start; node; node = node.parent) {
    if (node.name === 'String') return node.from < pos
    if (node.name === 'Object' || node.name === 'Array') return false
  }
  return false
}

/**
 * Whether the caret sits directly in an array with no element object around
 * it. Completing a bare `"id": ""` there would not be valid JSON.
 *
 * The element object counts as "inside", so the walk stops at an `Object` just
 * like it stops at a `Property`.
 */
function inBareArray(state: EditorState, pos: number): boolean {
  const start = syntaxTree(state).resolveInner(pos, -1)
  for (let node: typeof start | null = start; node; node = node.parent) {
    if (node.name === 'Array') return true
    if (node.name === 'Object' || isProperty(node.name)) return false
  }
  return false
}

/** Replace `[from, to)` and put the caret at `caret` inside the new text. */
function replaceWith(text: string, caret: number) {
  return (view: EditorView, _completion: Completion, from: number, to: number) => {
    view.dispatch({
      changes: { from, to, insert: text },
      selection: { anchor: from + caret },
      userEvent: 'input.complete',
    })
  }
}

/** A literal that needs no schema lookup, just a caret inside the text. */
function literal(label: string, type: string, detail: string, caret: number): Completion {
  return { label, type, detail, apply: replaceWith(label, caret) }
}

/** The kind of value a schema node accepts, after unwrapping `anyOf`/`$ref`. */
type LeafKind = 'string' | 'number' | 'boolean' | 'object' | 'array' | 'enum' | 'unknown'

/** Default value for a completed field, and where the caret lands inside it. */
function valueInsertion(kind: LeafKind, values: readonly string[]): { text: string; caret: number } {
  switch (kind) {
    case 'object':
      return { text: '{}', caret: 1 }
    case 'array':
      return { text: '[]', caret: 1 }
    case 'enum': {
      const first = values[0] ?? ''
      return { text: `"${first}"`, caret: 1 + Math.max(1, first.length - 1) }
    }
    case 'number':
      return { text: '0', caret: 0 }
    case 'boolean':
      return { text: 'false', caret: 0 }
    case 'string':
      return { text: '""', caret: 1 }
    default:
      return { text: 'null', caret: 0 }
  }
}

/**
 * Values worth offering for a field.
 *
 * `quoted` is false when the caret is already inside a string: the quotes are
 * part of the document there, so inserting `"dark"` would give `""dark""`.
 */
function valueCompletions(kind: LeafKind, values: readonly string[], nullable: boolean, quoted: boolean): Completion[] {
  if (kind === 'enum') {
    return values.map((value) => {
      const text = quoted ? `"${value}"` : value
      return {
        label: value,
        type: 'value',
        detail: nullable ? 'value or null' : 'value',
        apply: replaceWith(text, Math.min(text.length, quoted ? 1 + Math.max(1, value.length - 1) : value.length)),
      }
    })
  }
  switch (kind) {
    case 'boolean':
      return [literal('true', 'keyword', 'boolean', 4), literal('false', 'keyword', 'boolean', 5)]
    case 'string':
      return nullable
        ? [literal('""', 'text', 'empty string', 1), literal('null', 'keyword', 'no value', 4)]
        : [literal('""', 'text', 'empty string', 1)]
    case 'object':
      return [literal('{}', 'object', 'empty object', 1)]
    case 'array':
      return [literal('[]', 'array', 'empty array', 1)]
    case 'number':
      return [literal('0', 'number', 'number', 1)]
    default:
      return nullable ? [literal('null', 'keyword', 'no value', 4)] : []
  }
}

/** Field-name completion for the object enclosing the cursor. */
function keyCompletions(reader: JsonSchemaReader, node: JsonSchema, used: ReadonlySet<string>): Completion[] {
  const properties = reader.properties(node)
  return Object.keys(properties)
    .filter((name) => !used.has(name))
    .map((name) => {
      const field = properties[name]
      const kind = reader.leafKind(field)
      const values = reader.enumValues(field)
      const insertion = valueInsertion(kind, values)
      // `"name": value` — the caret ends up inside the value.
      const text = `${JSON.stringify(name)}: ${insertion.text}`
      const detail = descriptionOf(field) ?? reader.describe(field)
      return {
        label: name,
        type: 'property',
        detail,
        apply: replaceWith(text, name.length + 4 + insertion.caret),
      }
    })
}

/**
 * Schema-driven completion for the vault document.
 *
 * Exported so it can be exercised headlessly against a plain `EditorState`,
 * without standing up a DOM.
 */
export function vaultJsonCompletions(context: CompletionContext, reader: JsonSchemaReader): CompletionResult | null {
  const state = context.state
  // Nothing sensible fits between an array's brackets.
  if (inBareArray(state, context.pos)) return null

  const keys = enclosingKeys(state, context.pos)
  const word = context.matchBefore(/[\w-]*/)
  const from = word ? word.from : context.pos
  const options = { validFor: /^[\w-]*$/ }

  // Walk the document path into the schema. Arrays are transparent: a key
  // nested inside an array of objects belongs to the element.
  let node = reader.rootSchema
  for (const key of keys) {
    node = reader.concrete(node)
    if (reader.leafKind(node) === 'array') node = reader.concrete((node.items as JsonSchema) ?? {})
    if (reader.leafKind(node) !== 'object') return null
    const properties = reader.properties(node)
    if (!(key in properties)) return null
    node = properties[key]
  }

  // The last key may have landed on an array while the caret actually sits in
  // one of its element objects, so descend into the element.
  if (reader.leafKind(node) === 'array') {
    node = reader.concrete((node.items as JsonSchema) ?? {})
  }

  const resolved = reader.concrete(node)
  const kind = reader.leafKind(node)

  // A leaf at the caret means a value position: offer what the field accepts.
  if (kind !== 'object') {
    const quoted = !insideString(state, context.pos)
    // Inside an existing string only enumerated values are worth offering; a
    // free-text field has nothing to suggest there.
    if (!quoted && kind !== 'enum') return null
    return { ...options, from, options: valueCompletions(kind, reader.enumValues(node), reader.nullable(node), quoted) }
  }

  // Otherwise the caret is inside an object: offer its fields.
  return { ...options, from, options: keyCompletions(reader, resolved, new Set(siblingKeys(state, context.pos))) }
}

/**
 * Dismiss the popup when the caret moves away from it.
 *
 * Arrow keys are deliberately excluded: while the list is open they navigate
 * it, which is the usual editor behaviour. A click inside the document, or one
 * of the jump keys, is an unambiguous intent to move the caret instead.
 */
const dismissOnCaretMove = EditorView.domEventHandlers({
  mousedown: (_event, view) => {
    if (currentCompletions(view.state)) closeCompletion(view)
    return false
  },
  keyup: (event, view) => {
    if (!currentCompletions(view.state)) return false
    const jumps = ['Home', 'End', 'PageUp', 'PageDown']
    if (jumps.includes(event.key)) closeCompletion(view)
    return false
  },
})

/**
 * The editor: language, theme, history, search, folding and completion.
 *
 * `reader` wraps the schema fetched from the backend and is what the
 * completion source consults, so the document shape is never written down on
 * this side.
 */
export function vaultJsonEditor(reader: JsonSchemaReader, onSave: () => void): Extension {
  return [
    lineNumbers(),
    highlightActiveLineGutter(),
    highlightActiveLine(),
    highlightSelectionMatches(),
    drawSelection(),
    rectangularSelection(),
    history(),
    foldGutter(),
    indentOnInput(),
    bracketMatching(),
    closeBrackets(),
    EditorState.allowMultipleSelections.of(true),
    json(),
    // Without this the tokens render in generated, unstyleable classes and the
    // document looks like plain text.
    syntaxHighlighting(vaultHighlight),
    EditorView.lineWrapping,
    vaultJsonTheme,
    dismissOnCaretMove,
    autocompletion({
      override: [(context) => vaultJsonCompletions(context, reader)],
      // Opening stays explicit: Ctrl+Space, or the idle timer in the component.
      activateOnTyping: false,
    }),
    // Marks malformed JSON inline with the engine's own message.
    linter(jsonParseLinter()),
    keymap.of([
      // Saving is explicit; `preventDefault` also stops the webview's own
      // "save page" shortcut from firing.
      {
        key: 'Mod-s',
        preventDefault: true,
        run: () => {
          onSave()
          return true
        },
      },
      ...closeBracketsKeymap,
      // Tab indents. It comes before `defaultKeymap` so it wins, and it is not
      // part of `completionKeymap` — Tab never accepted a suggestion here, only
      // Enter does.
      { key: 'Tab', run: indentMore, shift: indentLess },
      ...defaultKeymap,
      ...searchKeymap,
      ...historyKeymap,
      ...foldKeymap,
      ...completionKeymap,
      ...lintKeymap,
    ]),
  ]
}
