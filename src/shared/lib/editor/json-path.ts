/** Where the caret is in the JSON: enclosing keys, sibling keys, string or bare array — read from the Lezer tree. */

import { syntaxTree } from '@codemirror/language'
import type { EditorState } from '@codemirror/state'

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
export function enclosingKeys(state: EditorState, pos: number): string[] {
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
export function siblingKeys(state: EditorState, pos: number): string[] {
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
export function insideString(state: EditorState, pos: number): boolean {
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
export function inBareArray(state: EditorState, pos: number): boolean {
  const start = syntaxTree(state).resolveInner(pos, -1)
  for (let node: typeof start | null = start; node; node = node.parent) {
    if (node.name === 'Array') return true
    if (node.name === 'Object' || isProperty(node.name)) return false
  }
  return false
}
