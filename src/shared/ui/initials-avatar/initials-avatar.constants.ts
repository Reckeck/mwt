/** `Alexey Petrov` -> `AP`; one word gives one letter, an empty name a dash. */
export function initialsOf(name: string): string {
  const words = name.trim().split(/\s+/).filter(Boolean)
  if (!words.length) return '-'
  const letters = words.length === 1 ? [words[0]] : [words[0], words[1]]
  return letters
    .map((word) => Array.from(word)[0])
    .join('')
    .toUpperCase()
}
