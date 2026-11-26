/**
 * Contrast of a validated custom theme (themes CI7).
 *
 * Runs the TC pairs on the resolved token set. `--foreground` on `--background` below 3:1 is a blocking
 * error; every other failing pair is a warning and sets `contrastWarning`. Pure, never throws: a colour
 * the contrast maths cannot read is skipped (the validator already rejects those).
 */
import { checkPairs, type ContrastPair, type FailingPair } from '../contrast'
import { CONTRAST_PAIRS } from '../contrast-pairs'
import type { ValidationIssue } from './validate'

const BLOCKING_FOREGROUND = '--foreground'
const BLOCKING_BACKGROUND = '--background'
const BLOCKING_MIN = 3

export interface ContrastReport {
  /** Blocking: the theme cannot be saved. */
  errors: ValidationIssue[]
  warnings: ValidationIssue[]
  /** All failing pairs with their ratios. */
  contrast: FailingPair[]
  contrastWarning: boolean
}

function failingPairs(tokens: Record<string, string>): FailingPair[] {
  return CONTRAST_PAIRS.flatMap((pair: ContrastPair) => {
    try {
      return checkPairs(tokens, [pair])
    } catch {
      return []
    }
  })
}

const isBlocking = (pair: FailingPair) =>
  pair.foreground === BLOCKING_FOREGROUND && pair.background === BLOCKING_BACKGROUND && pair.ratio < BLOCKING_MIN

function describePair(pair: FailingPair): string {
  return `${pair.foreground} on ${pair.background} is ${pair.ratio.toFixed(2)}:1, needs ${pair.min}:1`
}

function issue(pair: FailingPair, suffix = ''): ValidationIssue {
  return { line: 0, col: 0, message: `${describePair(pair)}${suffix}` }
}

export function reportContrast(tokens: Record<string, string>): ContrastReport {
  const contrast = failingPairs(tokens)
  const errors = contrast.filter(isBlocking).map((pair) => issue(pair, ` (minimum ${BLOCKING_MIN}:1 to save)`))
  const warnings = contrast.filter((pair) => !isBlocking(pair)).map((pair) => issue(pair))
  return { errors, warnings, contrast, contrastWarning: warnings.length > 0 }
}
