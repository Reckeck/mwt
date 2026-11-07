/**
 * Locale parts: `locales/parts/<slug>.{ru,en}.json`, one owner per file, so that parallel work does not
 * meet in `ru.json` and `en.json`. A part lays its flat keys over the main file. The runtime never
 * refuses a duplicate key: `duplicateLocaleKeys` is what `check-onboarding` fails on.
 */

export type LocaleMessages = Record<string, string>
/** Parts by file path, the shape `import.meta.glob` returns. */
export type LocaleParts = Record<string, LocaleMessages>

const fileName = (path: string) => path.slice(path.lastIndexOf('/') + 1)

function partsOf(lang: string, parts: LocaleParts): [string, LocaleMessages][] {
  return Object.entries(parts)
    .filter(([path]) => path.endsWith(`.${lang}.json`))
    .sort(([left], [right]) => left.localeCompare(right))
}

export function mergeLocale<Base extends LocaleMessages>(base: Base, lang: string, parts: LocaleParts): Base {
  return Object.assign({}, base, ...partsOf(lang, parts).map(([, messages]) => messages))
}

/** One line per key that two files of a language both define. */
export function duplicateLocaleKeys(base: LocaleMessages, lang: string, parts: LocaleParts): string[] {
  const owners = new Map(Object.keys(base).map((key) => [key, `${lang}.json`]))
  const found: string[] = []
  for (const [path, messages] of partsOf(lang, parts)) {
    const file = fileName(path)
    for (const key of Object.keys(messages)) {
      const owner = owners.get(key)
      if (owner) found.push(`i18n key "${key}" is in both ${owner} and ${file}`)
      else owners.set(key, file)
    }
  }
  return found
}
