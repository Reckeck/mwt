export type TabDef = { value: string; label: string }

export interface TabsProps {
  modelValue: string
  tabs: TabDef[]
  /**
   * Tabs behind «More ▾» at the end of the strip; while one of them is open
   * the button wears its label, so the strip still says where you are.
   */
  more?: TabDef[]
  /** The «More» button's label when none of `more` is open. */
  moreLabel?: string
}
