export interface AnalyticsReportViewProps {
  kind: ReportKind
  filter: ReportFilter
  /** The page states the facts itself, above several reports. */
  hideInsights?: boolean
}
