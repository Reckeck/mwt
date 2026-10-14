/**
 * What a mutation says when it is done. Read by the global `MutationCache`
 * handlers (`app/providers/query`), so a component describes the outcome once
 * and never calls `toast` itself.
 *
 * `success` (usually with `error`, optionally `undo`) or `silent` instead of
 * `success`; `check-ui` fails a `useMutation` that has neither. `silent` also
 * mutes the error toast: the mutation's own `onError` reports it then.
 */
export interface MutationNotice extends Record<string, unknown> {
  /**
   * The success toast: an i18n key (`'toast.budget.saved'`) or a function that
   * builds the text from the result and the variables (with the caller's `t`).
   */
  success?: string | ((data: unknown, variables: unknown) => string | NoticeText)
  /** The failure toast's title (an i18n key); the description is the backend's reason. */
  error?: string
  /** Why this mutation reports nothing itself (it shows the result on screen, or a richer toast). */
  silent?: string
  /** Offers "Отменить" in the success toast; runs this to put things back. */
  undo?: (data: unknown, variables: unknown) => Promise<unknown> | unknown
}

/** A toast's text with a muted second line (the name of what was saved, a count). */
export interface NoticeText {
  text: string
  description?: string
}

export interface NotifyOptions {
  /** A second, muted line. */
  description?: string
  /** A button in the toast ("Отменить", "Открыть"). */
  action?: { label: string; onClick: () => void }
}
