/** Ids of the preset baskets (`service/allocation.rs`); their names live in the locales until renamed. */
export const PRESET_CLASS_IDS = ['local', 'global', 'risky'] as const

/** What the user sees: the saved name, or the localized preset name while it is empty. */
export function assetClassName(cls: Pick<AssetClass, 'id' | 'name'>, translate: (key: string) => string): string {
  if (cls.name.trim()) return cls.name
  return (PRESET_CLASS_IDS as readonly string[]).includes(cls.id) ? translate(`assetClass.preset.${cls.id}`) : cls.id
}

/**
 * A hint, never an assignment: crypto goes to the risky part, cash in the
 * budget currency to the local market, other cash to the global one.
 * `null` when the preset basket it points at is not in the list.
 */
export function suggestClass(asset: AssetRef, budgetCurrency: string, classes: AssetClass[]): string | null {
  const local = asset.chain === 'fiat' && asset.symbol.toLowerCase() === budgetCurrency.toLowerCase()
  const id = asset.chain === 'fiat' ? (local ? 'local' : 'global') : 'risky'
  return classes.some((cls) => cls.id === id) ? id : null
}
