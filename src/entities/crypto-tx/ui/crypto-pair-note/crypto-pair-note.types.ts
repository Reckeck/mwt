export interface CryptoPairNoteProps {
  pair: CryptoPair
  /** Which end to show: `fiat` on an operation's card, `event` on a fiat record. */
  show: 'fiat' | 'event'
}
