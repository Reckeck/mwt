/** A recent vault as the list shows it: the path is the key, the name goes into the toast. */
export interface ForgetTarget {
  path: string
  name: string
}

/** What `forgetRecent` returned for one row, kept so `Undo` can put it back in place. */
export interface ForgottenRow {
  path: string
  index: number
}
