/** Commands: the user's own data sources (plan I2), as `service/datasources.rs` — without the network. */

import { resolve } from './core'
import type { MockCommands } from './core'

/** Currency → source id whose value is that currency's inflation; read by the rates mock. */
export const mockInflationSources: Record<string, string> = {}

const now = () => new Date().toISOString()

const PAGE = `<!doctype html><html><head><title>Ключевая ставка и инфляция</title></head><body>
<h1>Ключевая ставка и инфляция</h1><p>Ключевая ставка <b id="rate">16,50 %</b></p>
<table class="data"><tr><th>Дата</th><th>Ключевая ставка, %</th><th>Инфляция, % г/г</th></tr>
<tr><td>09.2026</td><td>14,00</td><td>6,35</td></tr><tr><td>08.2026</td><td>14,00</td><td>6,40</td></tr>
<tr><td>07.2026</td><td>15,00</td><td>6,52</td></tr></table></body></html>`

const sources: DataSource[] = [
  {
    id: 'src-demo',
    name: 'Инфляция ЦБ (таблица)',
    url: 'https://example.org/inflation',
    method: { kind: 'table', table_selector: 'table.data', column_header: 'Инфляция, % г/г', row: 'by_date' },
    output: 'number',
    number: { decimal_comma: true, multiplier: '1', strip: '' },
    interval: 'day',
    last: { at: now(), number: '6.35', text: null, series: [] },
    history: [],
    pending: null,
    failures: 0,
    status: 'ok',
    error: null,
    checked_at: now(),
    created_at: now(),
  },
]
const uses: Record<string, SourceUse[]> = {}

function view(source: DataSource): DataSourceView {
  const inflation = Object.entries(mockInflationSources)
    .filter(([, id]) => id === source.id)
    .map(([currency]): SourceUse => ({ kind: 'inflation', currency }))
  const others = (uses[source.id] ?? []).filter((use) => use.kind !== 'inflation')
  return { source, used_by: [...inflation, ...others] }
}

function find(id: unknown): DataSource {
  const source = sources.find((item) => item.id === id)
  if (!source) throw { code: 'NOT_FOUND', message: `data source ${String(id)}` }
  return source
}

/** A made-up reading: the mock has no network, so «Проверить» answers with a plausible number. */
function sample(draft: DataSourceDraft): Extracted {
  if (draft.output === 'series')
    return {
      number: null,
      text: null,
      series: [
        { label: '08.2026', value: '6.40' },
        { label: '09.2026', value: '6.35' },
      ],
      warning: null,
    }
  if (draft.output === 'text') return { number: null, text: 'Ключевая ставка 16,50 %', series: [], warning: null }
  return { number: '6.35', text: '6,35', series: [], warning: null }
}

export const sourcesCommands: MockCommands = {
  datasources_list: () => resolve(sources.map(view)),
  datasource_preview: ({ args }) => {
    const url = String(args?.url ?? '')
    if (!/^https:\/\//.test(url)) return Promise.reject({ code: 'INVALID_INPUT', message: 'https:// address expected' })
    if (url.endsWith('.json'))
      return resolve<SourcePreview>({
        kind: 'json',
        html: null,
        json: JSON.stringify({ data: { rate: 16.5, history: [6.4, 6.35] } }, null, 2),
        rows: [],
      })
    if (url.endsWith('.csv'))
      return resolve<SourcePreview>({
        kind: 'csv',
        html: null,
        json: null,
        rows: [
          ['Дата', 'Инфляция'],
          ['08.2026', '6,40'],
          ['09.2026', '6,35'],
        ],
      })
    return resolve<SourcePreview>({ kind: 'html', html: PAGE, json: null, rows: [] })
  },
  datasource_test: ({ args }) => resolve(sample(args?.draft as DataSourceDraft)),
  datasource_save: ({ args }) => {
    const draft = args?.draft as DataSourceDraft
    const existing = draft.id ? find(draft.id) : null
    const reading = sample(draft)
    const source: DataSource = {
      ...(existing ?? {
        id: `src-${Date.now()}`,
        last: { at: now(), number: reading.number, text: reading.text, series: reading.series },
        history: [],
        pending: null,
        failures: 0,
        status: 'ok',
        error: null,
        checked_at: now(),
        created_at: now(),
      }),
      name: draft.name,
      url: draft.url,
      method: draft.method,
      output: draft.output,
      number: draft.number,
      interval: draft.interval,
    }
    if (existing) sources.splice(sources.indexOf(existing), 1, source)
    else sources.push(source)
    for (const [currency, id] of Object.entries(mockInflationSources))
      if (id === source.id) delete mockInflationSources[currency]
    for (const use of draft.uses) if (use.kind === 'inflation') mockInflationSources[use.currency] = source.id
    uses[source.id] = draft.uses
    return resolve(view(source))
  },
  datasource_delete: ({ args }) => {
    sources.splice(sources.indexOf(find(args?.id)), 1)
    for (const [currency, id] of Object.entries(mockInflationSources))
      if (id === args?.id) delete mockInflationSources[currency]
    return resolve(undefined)
  },
  datasource_refresh: ({ args }) => {
    const source = find(args?.id)
    source.checked_at = now()
    source.status = 'ok'
    source.failures = 0
    source.error = null
    return resolve(view(source))
  },
  datasource_confirm: ({ args }) => {
    const source = find(args?.id)
    if (args?.accept && source.pending) source.last = source.pending
    source.pending = null
    source.status = 'ok'
    return resolve(view(source))
  },
  set_inflation_source: ({ args }) => {
    mockInflationSources[String(args?.currency ?? '')] = String(args?.source_id ?? '')
    return resolve(undefined)
  },
}
