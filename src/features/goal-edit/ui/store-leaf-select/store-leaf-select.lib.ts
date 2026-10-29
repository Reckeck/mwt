/** A group heading with the sub-accounts that can be picked under it. */
export interface LeafGroup {
  key: string
  /** The group's path (`Main bank`); empty for a top-level wallet without sub-accounts. */
  heading: string
  leaves: StoreTreeNode[]
}

/** Every node with no sub-accounts, in tree order, under the path of its parent. */
export function leafGroups(roots: StoreTreeNode[], search: string): LeafGroup[] {
  const needle = search.trim().toLowerCase()
  const groups = new Map<string, LeafGroup>()
  const visit = (node: StoreTreeNode) => {
    if (node.children.length) {
      node.children.forEach(visit)
      return
    }
    const heading = node.path.slice(0, -1).join(' › ')
    if (needle && !node.path.join(' ').toLowerCase().includes(needle)) return
    const group = groups.get(heading) ?? { key: heading, heading, leaves: [] }
    group.leaves.push(node)
    groups.set(heading, group)
  }
  roots.forEach(visit)
  return [...groups.values()]
}

/** The leaf with this id, wherever it sits in the tree. */
export function pickedLeaf(roots: StoreTreeNode[], id: string): StoreTreeNode | null {
  for (const node of roots) {
    if (node.store.id === id) return node
    const below = pickedLeaf(node.children, id)
    if (below) return below
  }
  return null
}
