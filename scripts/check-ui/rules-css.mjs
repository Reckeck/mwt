// Rules for the global stylesheet (src/**/*.css).

/** Every `@keyframes name { … }` block with its index. */
function keyframes(text) {
  const blocks = []
  for (const match of text.matchAll(/@keyframes\s+[\w-]+\s*\{/g)) {
    let depth = 0
    let end = match.index
    for (let index = match.index; index < text.length; index++) {
      if (text[index] === '{') depth++
      else if (text[index] === '}' && --depth === 0) {
        end = index
        break
      }
    }
    blocks.push({ index: match.index, body: text.slice(match.index, end) })
  }
  return blocks
}

export const CSS_RULES = [
  {
    // In Tailwind v4 --container-* also sizes max-w-sm/md/lg/…: overriding it resizes every dialog.
    id: 'theme-token-override',
    find: (text) => [...text.matchAll(/^\s*--container-(?:3xs|2xs|xs|sm|md|lg|xl|\dxl)\s*:/gm)],
    say: '--container-* overridden in @theme — it also changes max-w-sm/md/lg; name container sizes separately',
  },
  {
    // Tailwind v4 centres with the `translate` property; a keyframe `transform: translate` adds a second shift.
    id: 'transform-keyframes',
    find: (text) => keyframes(text).filter((block) => /\btransform\s*:[^;]*\btranslate/.test(block.body)),
    say: 'a keyframe animating transform: translate — it stacks on Tailwind translate-*; animate opacity / scale / translate',
  },
  {
    id: 'backdrop-blur',
    find: (text) => [...text.matchAll(/\bbackdrop-filter\s*:/g)],
    say: 'backdrop-filter — WebKitGTK stalls on it; use an opaque background',
  },
  {
    id: 'layout-transition',
    find: (text) =>
      [...text.matchAll(/\btransition(?:-property)?\s*:[^;]*\b(?:width|height|left|right|top|bottom|margin|padding|inset)\b/g)],
    say: 'a transition on a layout property relays the page every frame — animate transform or opacity',
  },
]
