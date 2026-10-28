// Helpers shared by the check-ui rule sets.

/** The text of a call starting at `open` (the index of its `(`), up to the matching `)`. */
export function callBody(text, open) {
  let depth = 0
  for (let index = open; index < text.length; index++) {
    const char = text[index]
    if (char === '(') depth++
    else if (char === ')') {
      depth--
      if (depth === 0) return text.slice(open, index + 1)
    }
  }
  return text.slice(open)
}

/** The `<template>` part of a `.vue` file with its offset, or null. */
export function templateOf(text) {
  const start = text.indexOf('<template>')
  if (start === -1) return null
  return { start, body: text.slice(start) }
}

/** Every `class="…"` attribute value (static) with its index. */
export function classAttrs(text) {
  return [...text.matchAll(/\bclass="([^"]*)"/g)]
}

/** Every opening tag `<Name …>` (attributes may span lines). */
export function openTags(text, name) {
  return [...text.matchAll(new RegExp(`<${name}\\b[^>]*>`, 'gs'))]
}

/** Shift template-relative matches back to file offsets. */
export const atFile = (matches, start) => matches.map((match) => Object.assign(match, { index: match.index + start }))

/**
 * The text with comments blanked out (same length, so indices and lines stay):
 * a rule must not fire on a comment that explains why the pattern is gone.
 */
export function blankComments(text) {
  const blank = (match) => match.replace(/[^\n]/g, ' ')
  return text
    .replace(/<!--[\s\S]*?-->/g, blank)
    .replace(/\/\*[\s\S]*?\*\//g, blank)
    .replace(/(^|[^:'"`\w])\/\/[^\n]*/g, (match, lead) => lead + blank(match.slice(lead.length)))
}
