/**
 * Which fields of a node make sense now: a value shows only the field of its
 * type, a schedule the day of the month only monthly, an operation the
 * «receive» fields only for an exchange.
 */
export function paramVisible(kind: string, name: string, params: Record<string, unknown>): boolean {
  const value = (key: string, fallback: string) => String(params[key] ?? fallback)
  if (kind === 'data.constant') {
    const type = value('value_type', 'number')
    if (name === 'value_type') return true
    if (name === 'number') return type === 'number' || type === 'money'
    if (name === 'currency') return type === 'money'
    return name === type
  }
  if (kind === 'trigger.schedule') {
    const every = value('every', 'month')
    if (name === 'day') return every === 'month'
    if (name === 'weekday') return every === 'week'
  }
  if (kind === 'action.transaction' && (name === 'to_asset' || name === 'to_amount')) {
    return value('tx_kind', 'transfer') === 'convert'
  }
  if (kind === 'convert.to') {
    if (name === 'currency') return value('to', 'number') === 'money'
    if (name === 'date_format') return value('to', 'number') === 'date'
  }
  return true
}

/** A short line about a node's fields for its card. */
export function nodeSummary(def: NodeDef | undefined, params: Record<string, unknown>, locale: string): string {
  if (!def) return ''
  const ru = locale.startsWith('ru')
  const pick = (name: string) => {
    const param = def.params.find((entry) => entry.name === name)
    const raw = params[name] ?? param?.default
    if (raw === null || raw === undefined || raw === '') return ''
    if (param?.kind.kind === 'select') {
      const option = param.kind.options.find(([key]) => key === raw)
      return option ? (ru ? option[1].ru : option[1].en) : String(raw)
    }
    if (typeof raw === 'object') {
      const record = raw as Record<string, unknown>
      return String(record.symbol ?? record.name ?? '')
    }
    return String(raw)
  }
  switch (def.kind) {
    case 'trigger.schedule': {
      const every = pick('every')
      const detail = params.every === 'day' ? '' : params.every === 'week' ? '' : `${pick('day')}-${ru ? 'е' : 'th'}`
      // A date moved off days off says so: `…, 09:00, one working day earlier`.
      const moved = params.workday && params.workday !== 'none' ? pick('workday') : ''
      return [every, detail, pick('time'), moved].filter(Boolean).join(', ')
    }
    case 'trigger.balance':
    case 'trigger.price':
      return `${pick('asset')} ${pick('condition')} ${pick('value')}`.trim()
    case 'logic.compare': {
      const op = pick('op')
      return op.replace('B', pick('b'))
    }
    case 'logic.math':
      return pick('op').replace('B', pick('b'))
    case 'logic.text':
      return pick('template')
    case 'logic.throttle':
      return `${pick('hours')} ${ru ? 'ч' : 'h'}`
    case 'var.get':
    case 'var.set':
    case 'var.add':
      return pick('name')
    case 'data.constant': {
      const type = String(params.value_type ?? 'number')
      if (type === 'money') return `${pick('number')} ${pick('currency').toUpperCase()}`
      return pick(type === 'number' ? 'number' : type)
    }
    case 'action.notify':
    case 'action.reminder':
      return pick('text')
    case 'action.transaction':
      return [pick('tx_kind'), pick('amount') !== '0' ? pick('amount') : '', pick('asset')].filter(Boolean).join(' · ')
    case 'action.stop':
    case 'logic.guard':
      return pick('reason')
    case 'trigger.event':
    case 'action.emit':
      return pick('name')
    case 'data.sum':
      return `${pick('flow')}, ${pick('period')}`
    case 'data.workday':
      return pick('calendar')
    case 'date.make': {
      const shift = Number(params.month_shift ?? 0)
      const month = shift ? (ru ? `, месяц +${shift}` : `, month +${shift}`) : ''
      return `${pick('day')}-${ru ? 'е' : 'th'}${month}, ${pick('time')}`
    }
    case 'date.add':
      return `+${pick('amount')} ${pick('unit')}`
    case 'date.boundary':
      return pick('which')
    case 'date.shift_workday':
      return pick('direction')
    case 'date.format':
      return pick('pattern')
    case 'text.extract':
    case 'text.replace':
      return pick(def.kind === 'text.extract' ? 'pattern' : 'find')
    case 'text.case':
      return pick('mode')
    case 'convert.to':
      return pick('to')
    case 'logic.round':
      return `${pick('mode')}, ${pick('places')}`
    case 'loop.repeat':
      return `× ${pick('count')}`
    case 'loop.range':
      return `${pick('from')} … ${pick('to')}`
    case 'list.sum':
    case 'list.filter':
    case 'text.join':
      return pick('field')
    default:
      return ''
  }
}
