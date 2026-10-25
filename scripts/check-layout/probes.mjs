// Code that runs inside the page (as async function bodies for `page.eval`).
// Each returns JSON: a list of problems as short strings.

/** Installed once per document: layout shifts and long tasks, readable and resettable. */
export const INSTALL_OBSERVERS = `
  if (window.__layout) return true
  window.__layout = { cls: 0, shifts: [], long: [] }
  new PerformanceObserver((list) => {
    for (const entry of list.getEntries()) {
      if (entry.hadRecentInput) continue
      window.__layout.cls += entry.value
      const node = entry.sources?.[0]?.node
      const what = node && node.nodeType === 1 ? node.tagName.toLowerCase() + '.' + String(node.className).trim().split(/\\s+/).slice(0, 3).join('.') : '?'
      window.__layout.shifts.push(what + ' ' + entry.value.toFixed(3))
    }
  }).observe({ type: 'layout-shift', buffered: true })
  try {
    new PerformanceObserver((list) => {
      for (const entry of list.getEntries()) window.__layout.long.push(Math.round(entry.duration))
    }).observe({ type: 'longtask', buffered: true })
  } catch {}
  return true
`

export const RESET_OBSERVERS = `window.__layout.cls = 0; window.__layout.shifts = []; window.__layout.long = []; return true`
export const READ_OBSERVERS = `return window.__layout`

/** A short, stable name for an element in a report. */
const DESCRIBE = `
  const describe = (el) => {
    const cls = typeof el.className === 'string' ? el.className.trim().split(/\\s+/).slice(0, 4).join('.') : ''
    const label = (el.getAttribute('aria-label') || el.getAttribute('data-tour') || el.textContent || '').replace(/\\s+/g, ' ').trim().slice(0, 30)
    return el.tagName.toLowerCase() + (cls ? '.' + cls : '') + (label ? ' "' + label + '"' : '')
  }
`

/** Horizontal page scroll, and elements wider than a container that does not scroll sideways. */
export const OVERFLOW = `
  ${DESCRIBE}
  const out = []
  // Touch hit areas (an invisible ::after past the box) are not content: measured without them.
  const quiet = document.createElement('style')
  quiet.textContent = '[class*="touch-hit"]::after { display: none !important }'
  document.head.append(quiet)
  const vw = document.documentElement.clientWidth
  if (document.documentElement.scrollWidth > vw + 1) out.push('page scrolls sideways: ' + document.documentElement.scrollWidth + ' > ' + vw)
  const scrollsX = (el) => { const ox = getComputedStyle(el).overflowX; return ox === 'auto' || ox === 'scroll' }
  const inScroller = (el) => { for (let node = el.parentElement; node; node = node.parentElement) if (scrollsX(node)) return true; return false }
  // A vertical scroller (overflow-y only) turns overflow-x into auto too: content wider than it is clipped, not scrolled on purpose.
  for (const el of document.querySelectorAll('[class*="overflow-y-"]')) {
    if (/overflow-x-/.test(el.className) || !scrollsX(el)) continue
    if (el.scrollWidth > el.clientWidth + 2) out.push('scroll area content ' + (el.scrollWidth - el.clientWidth) + 'px wider than its box: ' + describe(el))
  }
  for (const el of document.querySelectorAll('body *')) {
    if (el.closest('[data-layout-ignore], [data-sonner-toaster], canvas, svg, .vue-flow')) continue
    const cs = getComputedStyle(el)
    if (cs.display === 'none' || cs.visibility === 'hidden' || cs.position === 'fixed') continue
    const rect = el.getBoundingClientRect()
    if (!rect.width || !rect.height) continue
    if (inScroller(el)) continue
    if (rect.right > vw + 1 && cs.position !== 'absolute') out.push('past the window edge by ' + Math.round(rect.right - vw) + 'px: ' + describe(el))
    else if (el.children.length && !scrollsX(el) && el.scrollWidth > el.clientWidth + 2 && !/truncate|line-clamp/.test(el.className) && cs.textOverflow !== 'ellipsis' && cs.overflowX !== 'hidden' && cs.overflowX !== 'clip')
      out.push('content ' + (el.scrollWidth - el.clientWidth) + 'px wider than its box: ' + describe(el))
  }
  quiet.remove()
  return [...new Set(out)].slice(0, 12)
`

/** Rects of the rows a hover check watches (and of their direct children). */
export const ROW_RECTS = `
  const rows = [...document.querySelectorAll('[data-tx-row], [data-slot="table-row"], [data-stable-row]')]
    .filter((el) => el.getBoundingClientRect().height > 0).slice(0, 4)
  return rows.map((row) => {
    const box = row.getBoundingClientRect()
    const parts = [...row.querySelectorAll(':scope > *, :scope > * > *')].slice(0, 12).map((el) => {
      const rect = el.getBoundingClientRect()
      return [Math.round(rect.left), Math.round(rect.top), Math.round(rect.width), Math.round(rect.height)]
    })
    return { x: Math.round(box.left + box.width / 3), y: Math.round(box.top + Math.min(box.height / 2, 20)), parts }
  })
`

/** The open dialog panel's position. */
export const DIALOG_RECT = `
  const panel = document.querySelector('[role="dialog"][data-state="open"]')
  if (!panel) return null
  const rect = panel.getBoundingClientRect()
  return [Math.round(rect.left), Math.round(rect.top), Math.round(rect.width), Math.round(rect.height)]
`

/** On a touch screen: controls a finger can hardly hit, and actions it cannot see. */
export const TOUCH = `
  ${DESCRIBE}
  const out = []
  const visible = (el) => {
    for (let node = el; node; node = node.parentElement) {
      const cs = getComputedStyle(node)
      if (cs.display === 'none' || cs.visibility === 'hidden') return 'hidden'
      if (Number(cs.opacity) === 0) return 'transparent'
    }
    return 'shown'
  }
  // Fixed bars (bottom navigation, header) cover what scrolls under them: a control they touch
  // is measured where the user scrolls it, not half under the bar.
  const bars = [...document.querySelectorAll('body *')]
    .filter((node) => getComputedStyle(node).position === 'fixed' && node.getBoundingClientRect().height > 0)
    .map((node) => ({ node, rect: node.getBoundingClientRect() }))
  const underBar = (el, rect) =>
    bars.some(({ node, rect: bar }) => !node.contains(el) && rect.bottom > bar.top && rect.top < bar.bottom && rect.right > bar.left && rect.left < bar.right)
  // Half scrolled out of its scroller: measured where the user scrolls it.
  const clipped = (el, rect) => {
    for (let node = el.parentElement; node; node = node.parentElement) {
      const oy = getComputedStyle(node).overflowY
      if (oy !== 'auto' && oy !== 'scroll') continue
      const box = node.getBoundingClientRect()
      return rect.top < box.top || rect.bottom > box.bottom
    }
    return false
  }
  for (const el of document.querySelectorAll('button, [role="button"], [role="checkbox"], [role="switch"], [role="tab"], [role="menuitem"]')) {
    // [data-touch-inline]: a term inside a sentence, tapped like a link in text (WCAG 2.5.8 exception).
    if (el.closest('[data-layout-ignore], [data-sonner-toaster], .vue-flow, [aria-hidden="true"], [data-touch-inline]')) continue
    const rect = el.getBoundingClientRect()
    if (!rect.width || !rect.height) continue
    if (underBar(el, rect) || clipped(el, rect)) continue
    const state = visible(el)
    if (state === 'hidden') continue
    if (state === 'transparent') { out.push('invisible on touch: ' + describe(el)); continue }
    // The hit area may be larger than the box (a ::after inset), so ask the browser where taps land:
    // walk out from the centre each way until a tap no longer reaches the control. Neighbours
    // share the gap between them, so the span is measured, not assumed symmetric.
    const cx = rect.left + rect.width / 2, cy = rect.top + rect.height / 2
    const hit = (x, y) => { const at = document.elementFromPoint(x, y); return at && (at === el || el.contains(at)) }
    // Below the fold or under an overlay: measured on the view where it is on top.
    if (cy < 0 || cy > innerHeight || cx < 0 || cx > innerWidth || !hit(cx, cy)) continue
    const reach = (dx, dy) => { let step = 0; while (step < 40 && hit(cx + dx * (step + 1), cy + dy * (step + 1))) step++; return step }
    const spanX = rect.width >= 40 ? rect.width : reach(-1, 0) + reach(1, 0) + 1
    const spanY = rect.height >= 40 ? rect.height : reach(0, -1) + reach(0, 1) + 1
    // ~40px: 2px of slack for neighbours sharing the gap between them and for the 1px steps.
    if (spanX < 38 || spanY < 38) out.push('touch target ' + Math.round(spanX) + 'x' + Math.round(spanY) + ' (box ' + Math.round(rect.width) + 'x' + Math.round(rect.height) + '): ' + describe(el))
  }
  return [...new Set(out)].slice(0, 12)
`

/** Fields squeezed by their row: a value cannot be read or typed in a box this narrow. */
export const NARROW_FIELDS = `
  ${DESCRIBE}
  const out = []
  const fields = 'input:not([type="checkbox"]):not([type="radio"]):not([type="hidden"]):not([type="color"]), textarea, [role="combobox"]'
  for (const el of document.querySelectorAll(fields)) {
    if (el.closest('[data-layout-ignore], [data-sonner-toaster], .vue-flow, [aria-hidden="true"]')) continue
    const rect = el.getBoundingClientRect()
    if (!rect.height) continue
    const cs = getComputedStyle(el)
    if (cs.visibility === 'hidden' || Number(cs.opacity) === 0) continue
    // Below 4rem a sum or a select value is cut down to a couple of characters.
    if (rect.width < 64) out.push('field only ' + Math.round(rect.width) + 'px wide: ' + (el.id && document.querySelector('label[for="' + CSS.escape(el.id) + '"]')?.textContent?.trim() || '') + ' ' + describe(el))
  }
  return [...new Set(out)].slice(0, 12)
`
