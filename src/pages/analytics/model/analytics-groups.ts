/** The analytics sections: the overview and the three questions. */
export type AnalyticsGroup = 'overview' | 'flow' | 'wealth' | 'investments'

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
}

/** What the export button offers on each section (the flow view is added at runtime). */
export const GROUP_EXPORTS: Record<Exclude<AnalyticsGroup, 'flow'>, ReportKind[]> = {
  overview: [],
  wealth: ['networth', 'stores'],
  investments: ['portfolio', 'pnl', 'losses'],
}

export function flowViewOf(raw: unknown): FlowView {
  return FLOW_VIEWS.includes(raw as FlowView) ? (raw as FlowView) : 'months'
}
