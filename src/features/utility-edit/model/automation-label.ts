type T = (key: string, values?: Record<string, unknown>) => string

/**
 * An automation as the rent's «Платит автоматизация» lists it (Wallets-3 · 17):
 * «Аренда» · 10-го, Тинькофф → Хозяин. The route is there for a payment only.
 */
export function automationLabel(
  automation: Automation,
  t: T,
  storeName: (id: string) => string | undefined,
  contactName: (id: string) => string | undefined,
): string {
  const { schedule, template } = automation
  const when =
    'monthly' in schedule
      ? t('utilityEdit.onDay', { day: schedule.monthly.day })
      : 'weekly' in schedule
        ? t('schedule.weeklyOn', { weekday: t(`schedule.weekday.${schedule.weekly.weekday}`) }).toLowerCase()
        : t('schedule.everyNDays', { days: schedule.every_n_days.days }).toLowerCase()
  const head = `«${automation.name}» · ${when}`
  if (!('payment' in template)) return head
  const { store_id, to } = template.payment
  const from = storeName(store_id)
  const target = !to
    ? undefined
    : 'store' in to
      ? storeName(to.store)
      : 'contact' in to
        ? contactName(to.contact.contact_id)
        : to.raw
  if (!from) return head
  return `${head}, ${target ? `${from} → ${target}` : from}`
}
