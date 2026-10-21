/** The analytics sections: the overview, the three questions, the month's running balance and the checkup. */
export type AnalyticsGroup = 'overview' | 'flow' | 'wealth' | 'investments' | 'saldo' | 'health'

/** «Income and spending» shows one of three reports. */
export type FlowView = 'months' | 'year' | 'categories'

export const FLOW_VIEWS: FlowView[] = ['months', 'year', 'categories']

export const FLOW_VIEW_KIND: Record<FlowView, ReportKind> = {
  months: 'cashflow',
  year: 'year',
  categories: 'categories',
}

/** Route name of each section, in tab order. */
export const GROUP_ROUTE: Record<AnalyticsGroup, string> = {
  overview: 'analytics',
  flow: 'analytics-flow',
  wealth: 'analytics-wealth',
  investments: 'analytics-investments',
  saldo: 'analytics-saldo',
  health: 'analytics-health',
}

/** What the export button offers on each section (the flow view is added at runtime). */
export const GROUP_EXPORTS: Record<Exclude<AnalyticsGroup, 'flow'>, ReportKind[]> = {
  overview: [],
  wealth: ['networth', 'stores'],
  investments: ['portfolio', 'pnl', 'losses'],
  saldo: [],
  health: [],
}

/** Sections where «in today's money» means something; elsewhere the figures stay nominal. */
export const REAL_TERMS_GROUPS: AnalyticsGroup[] = ['flow', 'wealth']

/** Sections that show one month, picked with the overview's own `month` key. */
export const MONTH_GROUPS: AnalyticsGroup[] = ['overview', 'saldo']

/** Sections that judge «now» and take no filter: the bar is hidden there. */
export const UNFILTERED_GROUPS: AnalyticsGroup[] = ['health']

export function flowViewOf(raw: unknown): FlowView {
  return FLOW_VIEWS.includes(raw as FlowView) ? (raw as FlowView) : 'months'
}
