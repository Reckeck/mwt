/** Steps of the wizard, numbered as the canvas shows them: `Step 2 of 5`. */
export const RAISE_STEPS = ['warning', 'password', 'backup', 'reencrypt', 'done'] as const
export const RAISE_STAGES = ['backup', 'reencrypt', 'verify'] as const satisfies readonly RaiseStage[]

export const STEP_WARNING = 1
export const STEP_PASSWORD = 2
export const STEP_BACKUP = 3
export const STEP_RUN = 4
export const STEP_DONE = 5

/** What a failed run left: where it stopped and the backend's wording. */
export interface RaiseFailure {
  stage: RaiseStage
  message: string
}

export interface RaiseFieldErrors {
  pin?: string
  password?: string
  repeat?: string
}
