export interface CategorySelectProps {
  /** Groups to offer, in order (a transaction kind decides, see `categoryGroupsFor`). */
  groups: CategoryGroup[]
  /** Group a typed-in new category is created in; defaults to the first offered group. */
  createGroup?: CategoryGroup
  /** Several categories at once (transactions); a single key otherwise. */
  multiple?: boolean
  id?: string
  placeholder?: string
  ariaLabel?: string
  /** Turn off creating categories from the field. */
  noCreate?: boolean
}
