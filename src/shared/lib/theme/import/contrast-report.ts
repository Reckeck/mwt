/**
 * Contrast of a validated custom theme (themes CI7).
 *
 * Runs the TC pairs on the resolved token set. `--foreground` on `--background` below 3:1 is a blocking
 * error; every other failing pair is a warning and sets `contrastWarning`. Pure, never throws: a pair
 * whose colour the contrast maths cannot read is reported as unreadable, never skipped.
 */
import { checkPairs, ColorParseError, type ContrastPair, type FailingPair } from '../contrast'
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

interface PairScan {
  failing: FailingPair[]
  unreadable: ContrastPair[]
}

function scanPairs(tokens: Record<string, string>): PairScan {
  const scan: PairScan = { failing: [], unreadable: [] }
  for (const pair of CONTRAST_PAIRS) {
    try {
      scan.failing.push(...checkPairs(tokens, [pair]))
    } catch (error) {
      if (!(error instanceof ColorParseError)) throw error
      scan.unreadable.push(pair)
    }
  }
  return scan
}

const isBlockingPair = (pair: ContrastPair) =>
  pair.foreground === BLOCKING_FOREGROUND && pair.background === BLOCKING_BACKGROUND

const isBlocking = (pair: FailingPair) => isBlockingPair(pair) && pair.ratio < BLOCKING_MIN

const unreadableIssue = (pair: ContrastPair): ValidationIssue => ({
  line: 0,
  col: 0,
  message: `${pair.foreground} on ${pair.background} cannot be measured: a colour is unreadable`,
})

function describePair(pair: FailingPair): string {
  return `${pair.foreground} on ${pair.background} is ${pair.ratio.toFixed(2)}:1, needs ${pair.min}:1`
}

function issue(pair: FailingPair, suffix = ''): ValidationIssue {
  return { line: 0, col: 0, message: `${describePair(pair)}${suffix}` }
}

export function reportContrast(tokens: Record<string, string>): ContrastReport {
  const { failing: contrast, unreadable } = scanPairs(tokens)
  const errors = [
    ...contrast.filter(isBlocking).map((pair) => issue(pair, ` (minimum ${BLOCKING_MIN}:1 to save)`)),
    ...unreadable.filter(isBlockingPair).map(unreadableIssue),
  ]
  const warnings = [
    ...contrast.filter((pair) => !isBlocking(pair)).map((pair) => issue(pair)),
    ...unreadable.filter((pair) => !isBlockingPair(pair)).map(unreadableIssue),
  ]
  return { errors, warnings, contrast, contrastWarning: warnings.length > 0 }
}
