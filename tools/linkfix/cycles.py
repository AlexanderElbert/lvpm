#!/usr/bin/env python3
r"""Dependency cycles and stale class links in a LabVIEW source tree.

Reads the linker tables of every VI/ctl (LIvi/LIbd/LIfp/LIds), the private-data
and parent links inside each .lvclass, and the <Item URL=...> membership of
every .lvlib/.lvclass. Needs no LabVIEW.

    python tools/linkfix/cycles.py cycles <tree> [--json out.json]
    python tools/linkfix/cycles.py stale  <tree>

`cycles` reports strongly connected components between libraries and classes,
at two levels: innermost owner (class or lvlib) and top-level library. It also
prints how many files opening each top-level library loads, and links that
resolve to nothing in the tree or are absolute paths.

`stale` finds class/library links in a VI's LIvi table that are the ONLY reason
that class loads with the VI. LIvi lists what the VI as a whole links to;
LIbd/LIfp/LIds what its diagram, panel and data space use. A class in LIvi that
none of them points at is a candidate; most candidates are legitimate (a
typedef on the diagram contains the class), so every candidate edge is removed
and the class tested for reachability from what the VI really uses, under
LabVIEW's load rules. LabVIEW drops such a link when it saves the VI again.
Only in-tree classes are judged: vi.lib code cannot depend on project classes.

Load rules assumed (LabVIEW behaviour, not measured here): loading a member
loads its owning library; loading a class loads all its members; loading a
library loads the libraries nested in it; a class loads its parent and the
types in its private data.

Measured on NovaXC_Source (5178 files, ~10 s per command): `stale` found
Subsystem/Add Cluster to Attributes.vi carrying a leftover LIvi link to its
child class Subsystem.Interferometer, which made every one of 26 subsystem
classes load the Interferometer class (399 files instead of 260). Resaving the
VI removed it.
"""

import argparse
import html
import json
import os
import re
import sys
from collections import Counter, defaultdict
from pathlib import Path
from types import SimpleNamespace

sys.path.insert(0, str(Path(__file__).parent))
from deptree import pth0_records, TOKEN_SUBDIR  # noqa: E402
from linker_diff import rsrc_blocks  # noqa: E402
from linkobjs import li_objects  # noqa: E402

VI_EXT = {".vi", ".vim", ".vit", ".ctl", ".ctt"}
LIB_EXT = {".lvlib", ".lvclass"}
CONTAINER_EXT = {".lvlib", ".lvclass", ".llb", ".xctl", ".lvsc"}


def norm(p):
    return os.path.normcase(os.path.normpath(str(p)))


def decode_lvbin(s):
    """LabVIEW's XML `Bin` text encoding: chars '!'..'`' are 6-bit digits, MSB first."""
    acc = n = 0
    out = bytearray()
    for c in s:
        v = ord(c) - 0x21
        if not 0 <= v < 64:
            continue
        acc = ((acc << 6) | v) & 0xFFFFFF
        n += 6
        while n >= 8:
            n -= 8
            out.append((acc >> n) & 255)
    return bytes(out)


def resolve_pth(comps, base_file):
    """PTH0 components -> (path, None) or (None, external label).

    Relative paths start at the FILE: an empty component means "up one", so the
    first one lands in the file's folder. A path through an existing container
    file (X.lvclass/m.ctl) addresses a member of it; the container is returned.
    """
    head = comps[0]
    if head.lower() in TOKEN_SUBDIR:
        return None, head.lower() + "/" + "/".join(comps[1:3])
    if len(head) == 1 and head.isalpha():
        p = Path(head + ":\\", *comps[1:])
    else:
        p = Path(base_file)
        if head != "":
            p = p.parent
        for c in comps:
            p = p.parent if c == "" else p / c
    parts = p.parts
    for i, part in enumerate(parts[:-1]):
        # a FOLDER named X.lvclass exists in the wild; only a file is a container
        if Path(part).suffix.lower() in CONTAINER_EXT and Path(*parts[: i + 1]).is_file():
            return Path(*parts[: i + 1]), None
    return p, None


def class_bin_paths(text):
    """PTH0 records inside an .lvclass's encoded properties (private data, genealogy)."""
    out = set()
    for m in re.finditer(r'<Property Name="([^"]+)" Type="(Bin|Xml)">(.*?)</Property>', text, re.S):
        if m.group(1) == "NI.Lib.Icon":
            continue
        for blob in re.findall(r"[!-`]{16,}", html.unescape(m.group(3))):
            for shift in range(4):          # the blob's bit alignment is not fixed
                out.update(pth0_records(decode_lvbin(blob[shift:])))
    return out


def build(root):
    """Membership, parents and resolved dependencies of every file in a tree."""
    root = Path(root).resolve()
    files = [p for p in root.rglob("*") if p.is_file() and p.suffix.lower() in VI_EXT | LIB_EXT]
    exists = {norm(p): p for p in files}

    owner, members, parents, libtext = {}, defaultdict(set), defaultdict(set), {}
    multi_claim = defaultdict(set)
    for lib in (p for p in files if p.suffix.lower() in LIB_EXT):
        t = lib.read_text(encoding="utf-8-sig", errors="replace")
        lk = norm(lib)
        libtext[lk] = t
        for typ, url in re.findall(r'<Item Name="[^"]*" Type="([^"]*)" URL="([^"]*)"', t):
            tgt = norm(lib / html.unescape(url))       # URLs are relative to the library FILE
            if typ == "Parent":
                parents[lk].add(tgt)
                continue
            if typ == "Class Private Data" or typ.startswith("Friend"):
                continue
            if tgt in owner and owner[tgt] != lk:
                multi_claim[tgt] |= {owner[tgt], lk}
            owner[tgt] = lk
            members[lk].add(tgt)
    for k in [k for k, v in owner.items() if k == v]:
        del owner[k]

    deps, missing, external, errors = defaultdict(set), defaultdict(set), Counter(), []
    for p in files:
        f = norm(p)
        try:
            if p.suffix.lower() == ".lvclass":
                recs = class_bin_paths(libtext[f])
            elif p.suffix.lower() == ".lvlib":
                recs = set()
            else:
                recs = set()
                for k, secs in (rsrc_blocks(p.read_bytes()) or {}).items():
                    ident = k.decode(errors="replace") if isinstance(k, bytes) else str(k)
                    if ident.startswith("LI"):
                        for s in secs:
                            recs.update(pth0_records(s))
        except Exception as e:  # noqa: BLE001 - report and carry on
            errors.append((str(p), repr(e)))
            continue
        for comps in recs:
            tgt, ext = resolve_pth(comps, p)
            if ext:
                external[ext] += 1
                continue
            t = norm(tgt)
            if t == f:
                continue
            if t in exists:
                deps[f].add(t)
            elif not t.startswith(norm(root)):
                external["abs:" + "/".join(Path(tgt).parts[:3])] += 1
            else:
                missing[f].add(t)
    for c, ps in parents.items():
        deps[c] |= {p for p in ps if p in exists}

    def owner_chain(f):
        out, seen = [], {f}
        while f in owner and owner[f] not in seen:
            f = owner[f]
            seen.add(f)
            out.append(f)
        return out

    def top(f):
        chain = owner_chain(f)
        return chain[-1] if chain else f

    # graph with LabVIEW's load semantics
    load = defaultdict(set)
    for f, ts in deps.items():
        load[f] |= ts
    for f, o in owner.items():
        if f in exists:
            load[f].add(o)
    for lib, ms in members.items():
        for m in ms:
            if m in exists and (lib.endswith(".lvclass") or m.endswith((".lvclass", ".lvlib"))):
                load[lib].add(m)

    return SimpleNamespace(root=root, files=files, exists=exists, owner=owner, members=members,
                           multi_claim=multi_claim, deps=deps, missing=missing, external=external,
                           errors=errors, owner_chain=owner_chain, top=top, load=load)


def tarjan(g):
    index, low, onst, st, res, counter = {}, {}, set(), [], [], [0]
    sys.setrecursionlimit(max(10000, sys.getrecursionlimit()))

    def visit(v):
        index[v] = low[v] = counter[0]
        counter[0] += 1
        st.append(v)
        onst.add(v)
        for w in g.get(v, ()):
            if w not in index:
                visit(w)
                low[v] = min(low[v], low[w])
            elif w in onst:
                low[v] = min(low[v], index[w])
        if low[v] == index[v]:
            comp = []
            while True:
                w = st.pop()
                onst.discard(w)
                comp.append(w)
                if w == v:
                    break
            res.append(comp)

    for v in set(g) | {y for vs in g.values() for y in vs}:
        if v not in index:
            visit(v)
    return res


def closure(g, starts, stop=None):
    seen, st = set(starts), list(starts)
    while st:
        x = st.pop()
        if x == stop:
            continue
        for y in g[x]:
            if y not in seen:
                seen.add(y)
                st.append(y)
    return seen


def cmd_cycles(a):
    b = build(a.tree)
    rel = lambda x: os.path.relpath(x, b.root)  # noqa: E731
    is_lib = lambda x: x.endswith((".lvlib", ".lvclass"))  # noqa: E731

    def node(f, level):
        if level == "top":
            return b.top(f)
        return f if is_lib(f) else b.owner.get(f, f)

    out = {"files": len(b.files), "parse_errors": len(b.errors), "cycles": {}}
    for level in ("top", "inner"):
        g, why = defaultdict(set), defaultdict(list)
        for f, ts in b.deps.items():
            x = node(f, level)
            for t in ts:
                y = node(t, level)
                if x == y:
                    continue
                # pure containment (class <-> its own lvlib) is not a dependency
                if level == "inner" and (b.owner.get(x) == y or b.owner.get(y) == x) and (f == x or t == y):
                    continue
                g[x].add(y)
                if len(why[(x, y)]) < 3:
                    why[(x, y)].append((rel(f), rel(t)))
        sccs = sorted((s for s in tarjan(g) if len(s) > 1), key=len, reverse=True)
        out["cycles"][level] = [
            {"nodes": sorted(rel(n) for n in s),
             "edges": [{"from": rel(x), "to": rel(y), "because": why[(x, y)]}
                       for x in s for y in g[x] if y in set(s)]}
            for s in sccs]

    tops = sorted({b.top(f) for f in b.exists if is_lib(b.top(f))})
    out["load_size"] = sorted(((rel(t), len(closure(b.load, [t]))) for t in tops), key=lambda kv: -kv[1])
    out["missing"] = {rel(k): sorted(rel(x) for x in v) for k, v in b.missing.items()}
    out["absolute"] = {k: v for k, v in b.external.items() if k.startswith("abs:")}
    out["multi_claim"] = {rel(k): sorted(rel(x) for x in v) for k, v in b.multi_claim.items()}

    print(f"{out['files']} files, {out['parse_errors']} unparsed")
    for level, title in (("top", "top-level library"), ("inner", "class / innermost library")):
        cs = out["cycles"][level]
        print(f"\n== {title} cycles: {len(cs)} ==")
        for c in cs:
            print(f"  {len(c['nodes'])}: " + ", ".join(c["nodes"]))
            for e in c["edges"]:
                print(f"      {e['from']} -> {e['to']}   e.g. {e['because'][0][0]} links {e['because'][0][1]}")
    print("\n== files loaded when opening a top-level library (largest 15) ==")
    for k, v in out["load_size"][:15]:
        print(f"  {v:5}  {k}")
    print(f"\nfiles with in-tree links that resolve to nothing: {len(out['missing'])}")
    print(f"absolute links leaving the tree: {out['absolute']}")
    if out["multi_claim"]:
        print(f"files claimed by more than one library: {len(out['multi_claim'])}")
    if a.json:
        Path(a.json).write_text(json.dumps(out, indent=1))
    return 0


def cmd_stale(a):
    b = build(a.tree)
    rel = lambda x: os.path.relpath(x, b.root)  # noqa: E731

    def targets(objs, vi):
        out = []
        for ident, qual, (_tp, comps), *_ in objs:
            comps = [c.decode("latin-1") for c in comps]
            if not comps:
                continue
            p, ext = resolve_pth(comps, vi)
            if not ext:
                out.append((ident.decode(), ":".join(q.decode("latin-1", "replace") for q in qual), norm(p)))
        return out

    cands = []
    for f, p in b.exists.items():
        if p.suffix.lower() not in VI_EXT:
            continue
        try:
            _, res = li_objects(p.read_bytes())
        except Exception:  # noqa: BLE001 - not every ctl parses; skip it
            continue
        used = set()
        for blk in ("LIbd", "LIfp", "LIds"):
            used |= {c for _, _, c in targets(res.get(blk, []), p)}
        livi = targets(res.get("LIvi", []), p)
        # the owner the VI records itself (VILB) and the one the library XML records
        chain = set(b.owner_chain(f))
        for ident, _, c in livi:
            if ident == "VILB":
                chain |= {c, *b.owner_chain(c)}
        for ident, qual, c in livi:
            if ident == "VILB" or not c.endswith((".lvclass", ".lvlib")):
                continue
            if c in used or c in chain or c not in b.exists:
                continue
            cands.append((f, ident, qual, c, used | chain))

    for f, _, _, c, _ in cands:      # judge every candidate without any of them
        b.load[f].discard(c)
    stale = [(f, i, q, c) for f, i, q, c, starts in cands if c not in closure(b.load, starts, stop=f)]

    print(f"{len(cands)} candidate LIvi class links; {len(stale)} stale "
          "(the class loads with the VI only because of this link)")
    by_class = defaultdict(list)
    for f, i, q, c in stale:
        by_class[c].append((f, i, q))
    for c, hs in sorted(by_class.items(), key=lambda kv: -len(kv[1])):
        print(f"\n{rel(c)}   <- {len(hs)} VI(s)")
        for f, i, q in hs:
            print(f"    {rel(f)}   [{i} {q}]")
    return 1 if stale else 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    sub = ap.add_subparsers(dest="cmd", required=True)
    c = sub.add_parser("cycles", help="library/class dependency cycles and load sizes")
    c.add_argument("tree")
    c.add_argument("--json", help="write the full report here")
    s = sub.add_parser("stale", help="LIvi class links that are the only reason a class loads")
    s.add_argument("tree")
    a = ap.parse_args()
    return {"cycles": cmd_cycles, "stale": cmd_stale}[a.cmd](a)


if __name__ == "__main__":
    sys.exit(main())
