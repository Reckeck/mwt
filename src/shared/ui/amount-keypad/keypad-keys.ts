export type KeypadKey =
  | { kind: 'digit'; id: string; value: string; label: string }
  | { kind: 'operator'; id: string; value: '+' | '-' | '*' | '/'; label: string }
  | { kind: 'decimal'; id: string }
  | { kind: 'backspace'; id: string }

const digit = (value: string): KeypadKey => ({ kind: 'digit', id: value, value, label: value })
const operator = (value: '+' | '-' | '*' | '/', label: string): KeypadKey => ({
  kind: 'operator',
  id: value,
  value,
  label,
})

/** Digits on three columns, operators on the fourth. */
export const KEYPAD_ROWS: KeypadKey[][] = [
  [digit('7'), digit('8'), digit('9'), operator('+', '+')],
  [digit('4'), digit('5'), digit('6'), operator('-', '-')],
  [digit('1'), digit('2'), digit('3'), operator('*', 'x')],
  [{ kind: 'decimal', id: 'decimal' }, digit('0'), { kind: 'backspace', id: 'backspace' }, operator('/', '/')],
]
