export type TabDef = { value: string; label: string }

export interface TabsProps {
  modelValue: string
  tabs: TabDef[]
}
