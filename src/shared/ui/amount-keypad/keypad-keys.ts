export type KeypadOperator = '+' | '-' | '*' | '/'

export type KeypadKey =
  | { kind: 'digit'; id: string; value: string; label: string }
  | { kind: 'operator'; id: string; value: KeypadOperator; label: string }
  | { kind: 'decimal'; id: string }
  | { kind: 'backspace'; id: string }

const digit = (value: string): KeypadKey => ({ kind: 'digit', id: value, value, label: value })
const operator = (value: KeypadOperator, label: string): KeypadKey => ({
  kind: 'operator',
  id: value,
  value,
  label,
})

/** A phone's order (board 463195): 1 2 3 on top, the operators down the fourth column, `+` last. */
export const KEYPAD_ROWS: KeypadKey[][] = [
  [digit('1'), digit('2'), digit('3'), operator('/', '÷')],
  [digit('4'), digit('5'), digit('6'), operator('*', '×')],
  [digit('7'), digit('8'), digit('9'), operator('-', '−')],
  [{ kind: 'decimal', id: 'decimal' }, digit('0'), { kind: 'backspace', id: 'backspace' }, operator('+', '+')],
]
