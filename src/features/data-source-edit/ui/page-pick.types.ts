/** What a click in the page preview gives the form. */
export interface PagePick {
  selector: string
  text: string
  hint: string
  /** Set when the element is a table cell: the table and its column header. */
  table: { selector: string; header: string } | null
}
