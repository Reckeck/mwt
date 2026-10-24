import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { formatMoney } from '@/shared/lib/format'
import { txKind, txLegs } from '@/shared/lib/transactions'
import { outcomeQuestion } from '@/shared/lib/workflow'

/**
 * The words of a run report shared by the panel and the phone's card: the
 * question an emulation stopped at, the answers given so far and the
 * operations it made or would make, in one line.
 */
export function useRunSummary(report: () => RunReport, answers: () => Record<string, boolean> | undefined) {
  const { t } = useI18n()

  /** An emulation that stopped at «Ждать подтверждения»: answered right here. */
  const question = computed(() => (report().run.dry_run ? outcomeQuestion(report().run.outcome) : null))
  const answered = computed(() =>
    Object.values(answers() ?? {})
      .map((yes) => t(yes ? 'workflow.runPanel.yes' : 'workflow.runPanel.no'))
      .join(', '),
  )

  function line(tx: Transaction): string {
    const leg = txLegs(tx).find((entry) => Number(entry.amount) < 0) ?? txLegs(tx)[0]
    const amount = leg ? formatMoney(String(Math.abs(Number(leg.amount))), { currency: leg.asset.symbol }) : ''
    return `${t(`tx.kind.${txKind(tx)}`)} ${amount}`.trim()
  }
  /** «Создала бы операций: 1 · Перевод 157,50 $.» */
  const made = computed(() => {
    const list = report().transactions
    if (!list.length) return ''
    const key = report().run.dry_run ? 'workflow.runPanel.wouldMakeLine' : 'workflow.runPanel.madeLine'
    return t(key, { count: list.length, list: list.map(line).join('; ') })
  })

  return { question, answered, made }
}
