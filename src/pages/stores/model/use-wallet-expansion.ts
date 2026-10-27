import { useStorage } from '@vueuse/core'

/** One line of the wallets list: a wallet, or a sub-account under an expanded one. */
export interface WalletLine {
  node: StoreTreeNode
  /** 0 for a top-level wallet. */
  depth: number
  /** Part of the parent's subtree value, 0..1; `null` for a top-level wallet. */
  share: number | null
}

/**
 * Which wallets show their sub-accounts. Folded by default; the choice is a
 * setting of this device, kept in local storage (`useStorage` survives it
 * being blocked).
 */
export function useWalletExpansion() {
  const opened = useStorage<string[]>('wallets-expanded', [])

  const isOpen = (id: string) => opened.value.includes(id)
  const toggle = (id: string) => {
    opened.value = isOpen(id) ? opened.value.filter((known) => known !== id) : [...opened.value, id]
  }

  /** The roots in order, each followed by its expanded sub-accounts, depth first. */
  function lines(roots: StoreTreeNode[]): WalletLine[] {
    const walk = (nodes: StoreTreeNode[], depth: number, parent: StoreTreeNode | null): WalletLine[] =>
      nodes.flatMap((node) => {
        const total = Number(parent?.subtree_base ?? 0)
        const share = parent ? (total ? Number(node.subtree_base) / total : 0) : null
        return [{ node, depth, share }, ...(isOpen(node.store.id) ? walk(node.children, depth + 1, node) : [])]
      })
    return walk(roots, 0, null)
  }

  return { isOpen, toggle, lines }
}
