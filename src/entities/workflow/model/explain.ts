import { outcomeDetail, outcomeKey } from '@/shared/lib/workflow'

type Translate = (key: string, values?: Record<string, unknown>) => string

/**
 * A run in a few sentences: what started it, how far it went, what it made
 * (or would make), and why it ended where it did.
 */
export function explainRun(report: RunReport, t: Translate, locale: string): string {
  const ru = locale.startsWith('ru')
  const steps = report.run.trace
  const first = steps[0]
  const sentences: string[] = []
  if (first) sentences.push(ru ? `${first.text_ru}.` : `${first.text_en}.`)
  sentences.push(t('workflow.explain.steps', { count: steps.length }))
  const skipped = steps.filter((step) => step.status === 'skipped').length
  if (skipped) sentences.push(t('workflow.explain.skipped', { count: skipped }))
  const dry = report.run.dry_run
  if (report.transactions.length) {
    sentences.push(
      t(dry ? 'workflow.explain.wouldMake' : 'workflow.explain.made', { count: report.transactions.length }),
    )
  }
  const queued = report.queued.reduce((sum, batch) => sum + batch.transactions.length, 0)
  if (queued) sentences.push(t('workflow.explain.queued', { count: queued }))
  if (report.notices.length) {
    sentences.push(
      t(dry ? 'workflow.explain.wouldNotify' : 'workflow.explain.notified', { count: report.notices.length }),
    )
  }
  if (!report.transactions.length && !queued && !report.notices.length && outcomeKey(report.run.outcome) === 'done') {
    sentences.push(t('workflow.explain.nothing'))
  }
  const key = outcomeKey(report.run.outcome)
  // A question the node did not word is shown in the app's own words.
  const raw = outcomeDetail(report.run.outcome)
  const detail =
    key === 'dropped' ? t(`workflow.dropped.${raw}`) : raw || (key === 'asking' ? t('workflow.defaultQuestion') : '')
  if (key !== 'done') sentences.push(t(`workflow.explain.${key}`, { detail }))
  return sentences.join(' ')
}
