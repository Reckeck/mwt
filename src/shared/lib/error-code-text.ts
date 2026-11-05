/** The text of a backend error code (`errors.<CODE>`); a code without text reads as the unknown error. */
export function errorCodeText(code: string, t: (key: string) => string, te: (key: string) => boolean): string {
  return t(te(`errors.${code}`) ? `errors.${code}` : 'errors.UNKNOWN')
}
