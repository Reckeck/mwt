export type Counts = Map<string, number>
export interface UiScan {
  counts: Counts
  firstHit: Map<string, string>
  classFiles: Map<string, Set<string>>
}
export const SAY: Record<string, string>
export const RULE_IDS: string[]
export function scanUi(root: string): UiScan
export function judgeUi(
  scan: UiScan,
  baseline: Record<string, number>,
): { errors: { rule: string; text: string }[]; improved: string[] }
export function nextUiBaseline(scan: UiScan, baseline: Record<string, number>): Record<string, number>
export function copiedClasses(scan: UiScan): [string, Set<string>][]
