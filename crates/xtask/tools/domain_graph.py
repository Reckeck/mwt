# domain_graph.py: prints the module dependency graph and its cycles for a Rust crate.
# Run: python3 crates/xtask/tools/domain_graph.py crates/core/src
# Not part of `xtask check`; kept here as an analysis aid.
import os, re, sys, collections
root = sys.argv[1]
impl_re = re.compile(r'impl(?:<[^>]*>)?\s+UnlockedVault\s*\{')
fn_re = re.compile(r'\n\s*(?:pub(?:\([^)]*\))?\s+)?fn\s+([a-z_][a-z0-9_]*)')
def group_of(rel):
    first = rel.split('/')[0]
    return first[:-3] if first.endswith('.rs') else first
files = {}
for dirpath, _, names in os.walk(root):
    for name in names:
        if name.endswith('.rs') and 'test' not in name:
            path = os.path.join(dirpath, name)
            rel = os.path.relpath(path, root)
            if '/tests/' in '/' + rel: continue
            text = open(path).read()
            text = text.split('#[cfg(test)]')[0]
            files[rel] = text
owner = {}
for rel, text in files.items():
    for match in impl_re.finditer(text):
        depth = 0; start = match.end() - 1
        for index in range(start, len(text)):
            if text[index] == '{': depth += 1
            elif text[index] == '}':
                depth -= 1
                if depth == 0: break
        body = text[start:index]
        for name in fn_re.findall(body):
            owner.setdefault(name, set()).add(group_of(rel))
# unique owners only
owner = {name: next(iter(groups)) for name, groups in owner.items() if len(groups) == 1}
edges = collections.Counter()
size = collections.Counter()
for rel, text in files.items():
    group = group_of(rel)
    size[group] += text.count('\n')
    for name in re.findall(r'(?:self|vault|v)\.([a-z_][a-z0-9_]*)\(', text):
        target = owner.get(name)
        if target and target != group:
            edges[(group, target)] += 1
# SCCs (Tarjan)
graph = collections.defaultdict(set)
for (src, dst) in edges: graph[src].add(dst)
nodes = set(size) | set(graph)
index_of = {}; low = {}; stack = []; on = set(); sccs = []; counter = [0]
sys.setrecursionlimit(10000)
def visit(node):
    index_of[node] = low[node] = counter[0]; counter[0] += 1; stack.append(node); on.add(node)
    for nxt in graph[node]:
        if nxt not in index_of: visit(nxt); low[node] = min(low[node], low[nxt])
        elif nxt in on: low[node] = min(low[node], index_of[nxt])
    if low[node] == index_of[node]:
        comp = []
        while True:
            top = stack.pop(); on.discard(top); comp.append(top)
            if top == node: break
        sccs.append(comp)
for node in sorted(nodes):
    if node not in index_of: visit(node)
big = [comp for comp in sccs if len(comp) > 1]
print("groups:", len(nodes), " cross-group call edges:", len(edges))
for comp in sorted(big, key=lambda comp: -sum(size[name] for name in comp)):
    print(f"CYCLE ({len(comp)} groups, {sum(size[name] for name in comp)} lines):", ' '.join(sorted(comp, key=lambda name: -size[name])))
acyclic = [comp[0] for comp in sccs if len(comp) == 1]
print("\nnot in any cycle (lines):", ' '.join(f"{name}({size[name]})" for name in sorted(acyclic, key=lambda name: -size[name])))
print("\nmost-called groups (incoming edges by count):")
incoming = collections.Counter()
for (src, dst), count in edges.items(): incoming[dst] += count
for name, count in incoming.most_common(12): print(f"  {name}: {count} calls from {sum(1 for (s,d) in edges if d==name)} groups")
print("\n== edges inside the big cycle (src -> dst: count, called fns)")
comp = set(max(big, key=lambda comp: sum(size[name] for name in comp)))
calls = collections.defaultdict(collections.Counter)
for rel, text in files.items():
    group = group_of(rel)
    for name in re.findall(r'(?:self|vault|v)\.([a-z_][a-z0-9_]*)\(', text):
        target = owner.get(name)
        if target and target != group and group in comp and target in comp:
            calls[(group, target)][name] += 1
for (src, dst), counter in sorted(calls.items()):
    print(f"{src} -> {dst}: {sum(counter.values())}  {', '.join(n for n,_ in counter.most_common(4))}")
