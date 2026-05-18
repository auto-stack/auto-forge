#!/usr/bin/env python3
"""
Spec regeneration script for auto-forge.
Cleans duplicates, regenerates sequential IDs, adds stack/module tags.
"""

import re
import os
from datetime import datetime, timezone

SPECS_DIR = "specs"

# ── Goal tag mapping ──────────────────────────────────────────────────────────
GOAL_TAGS = {
    "G1":    ("stack:both",     "module:agent-config"),
    "G2":    ("stack:backend",  "module:relay"),
    "G3":    ("stack:backend",  "module:relay"),
    "G4":    ("stack:backend",  "module:relay"),
    "G5":    ("stack:backend",  "module:relay"),
    "G6":    ("stack:both",     "module:relay"),
    "G7":    ("stack:both",     "module:specs"),
    "G8":    ("stack:both",     "module:chat"),
    "G8.1":  ("stack:frontend", "module:chat"),
    "G8.2":  ("stack:frontend", "module:chat"),
    "G8.3":  ("stack:frontend", "module:chat"),
    "G8.4":  ("stack:frontend", "module:chat"),
    "G9":    ("stack:both",     "module:specs"),
    "G9.1":  ("stack:frontend", "module:specs"),
    "G9.2":  ("stack:frontend", "module:specs"),
    "G9.3":  ("stack:frontend", "module:specs"),
    "G9.4":  ("stack:both",     "module:specs"),
    "G10":   ("stack:frontend", "module:relay"),
    "G10.1": ("stack:frontend", "module:relay"),
    "G10.2": ("stack:frontend", "module:relay"),
    "G10.3": ("stack:frontend", "module:relay"),
    "G10.4": ("stack:frontend", "module:relay"),
    "G11":   ("stack:frontend", "module:ui-system"),
    "G11.1": ("stack:frontend", "module:ui-system"),
    "G11.2": ("stack:frontend", "module:ui-system"),
    "G11.3": ("stack:frontend", "module:ui-system"),
    "G11.4": ("stack:frontend", "module:ui-system"),
    "G12":   ("stack:both",     "module:project"),
    "G13":   ("stack:both",     "module:api-sources"),
    "G14":   ("stack:backend",  "module:provider"),
    "G15":   ("stack:backend",  "module:cli"),
    "G16":   ("stack:both",     "module:chat"),
    "G16.1": ("stack:both",     "module:chat"),
    "G16.2": ("stack:frontend", "module:chat"),
    "G17":   ("stack:both",     "module:chat"),
    "G17.1": ("stack:backend",  "module:chat"),
    "G17.2": ("stack:backend",  "module:chat"),
    "G17.3": ("stack:frontend", "module:chat"),
    "G18":   ("stack:both",     "module:wiki"),
    "G19":   ("stack:frontend", "module:editor"),
    "G19.1": ("stack:frontend", "module:editor"),
    "G19.2": ("stack:frontend", "module:editor"),
    "G19.3": ("stack:frontend", "module:editor"),
    "G20":   ("stack:both",     "module:wiki"),
    "G21":   ("stack:frontend", "module:specs"),
    "G22":   ("stack:both",     "module:errand"),
    "G23":   ("stack:both",     "module:relay"),
}

# ── Helpers ───────────────────────────────────────────────────────────────────

def read_file(path):
    with open(path, "r", encoding="utf-8") as f:
        return f.read()

def write_file(path, content):
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)

def parse_sections(text, prefix):
    """Parse text into (preamble, list_of_sections).
    Sections start with ## <PREFIX><digits>..."""
    regex = re.compile(rf"^(## {prefix}\d+(?:\.\d+)?[:\s].*)$", re.MULTILINE)
    matches = list(regex.finditer(text))
    if not matches:
        return text, []
    preamble = text[:matches[0].start()]
    sections = []
    for i, m in enumerate(matches):
        start = m.start()
        end = matches[i + 1].start() if i + 1 < len(matches) else len(text)
        header = m.group(1)
        body = text[start + len(header):end]
        id_match = re.search(rf"^## ({prefix}\d+(?:\.\d+)?)", header)
        sid = id_match.group(1) if id_match else None
        title = re.sub(rf"^## {prefix}\d+(?:\.\d+)?[:\s]*", "", header).strip()
        sections.append({"id": sid, "title": title, "header": header, "body": body})
    return preamble, sections

def split_at_second_first_id(sections, first_id):
    """Split sections into (first_copy, second_copy) at second occurrence of first_id."""
    idx = None
    for i, s in enumerate(sections):
        if s["id"] == first_id:
            if idx is None:
                idx = i  # first occurrence
            else:
                return sections[:i], sections[i:]
    return sections, []

def add_tags_to_goal_section(section_text, goal_id):
    """Insert **Tags:** line after Status or Depends on."""
    tags = GOAL_TAGS.get(goal_id)
    if not tags:
        return section_text
    tags_line = "**Tags:** " + ", ".join(tags) + "\n"
    if "**Tags:**" in section_text:
        return section_text
    for pat in [r"(\*\*Depends on:\*\*.*?\n)", r"(\*\*Status:\*\*.*?\n)"]:
        m = re.search(pat, section_text)
        if m:
            insert_pos = m.end()
            return section_text[:insert_pos] + tags_line + section_text[insert_pos:]
    return section_text


# ── Process Goals ─────────────────────────────────────────────────────────────

def process_goals():
    text = read_file(os.path.join(SPECS_DIR, "goals.ad"))

    # Find all goal sections (## G or ### G)
    goal_regex = re.compile(r"^(##?#? G\d+(?:\.\d+)?[:\s].*)$", re.MULTILINE)
    matches = list(goal_regex.finditer(text))

    sections = []
    for idx, m in enumerate(matches):
        start = m.start()
        end = matches[idx + 1].start() if idx + 1 < len(matches) else len(text)
        block = text[start:end]
        header_line = m.group(1)
        idm = re.search(r"G\d+(?:\.\d+)?", header_line)
        gid = idm.group(0) if idm else None
        title = re.sub(r"^#+ G\d+(?:\.\d+)?[:\s]*", "", header_line).strip()
        sections.append({"id": gid, "title": title, "block": block, "header": header_line})

    # Keep longest block per ID
    by_id = {}
    for s in sections:
        if s["id"] in by_id:
            if len(s["block"]) > len(by_id[s["id"]]["block"]):
                by_id[s["id"]] = s
        else:
            by_id[s["id"]] = s

    # Create G20 if missing
    if "G20" not in by_id:
        g20_block = (
            "## G20 Wiki Search & Tagging System provides VitePress/VSCode-style context-aware full-text search\n"
            "**Status:** proposed\n"
            "**Depends on:** G18\n"
            "**Tags:** stack:both, module:wiki\n\n"
            "- [ ] Full-text search across wiki pages with relevance ranking\n"
            "- [ ] Hierarchical tag system with `kind:value` format\n"
            "- [ ] Tag cloud navigation for browsing by topic\n"
            "- [ ] Search indexing for raw file content (PDF text extraction)\n"
            "- [ ] Context-aware search scoped to current project\n\n"
        )
        by_id["G20"] = {"id": "G20", "title": "Wiki Search & Tagging System provides VitePress/VSCode-style context-aware full-text search",
                         "block": g20_block}

    # Sort by ID
    def sort_key(gid):
        parts = gid.split(".")
        return (int(parts[0][1:]), tuple(int(p) for p in parts[1:]))

    sorted_ids = sorted(by_id.keys(), key=sort_key)

    # Rebuild preamble (everything before first section)
    preamble = text[:matches[0].start()]
    # Remove AI append noise
    preamble = re.sub(r"\nPerfect! Here's the exact content to append to `specs/goals.ad`:\n", "\n", preamble)
    preamble = re.sub(r"\nLet me know once you've added these and fixed the bug, and I can continue with the next phase of planning!\n", "\n", preamble)

    out = preamble.rstrip() + "\n"
    tag_count = 0
    for gid in sorted_ids:
        s = by_id[gid]
        block = s["block"]
        # Ensure correct header level
        block = re.sub(r"^#+ G\d+(?:\.\d+)?[:\s].*\n", f"## {gid} {s['title']}\n", block, count=1)
        block_before = block
        block = add_tags_to_goal_section(block, gid)
        if "**Tags:**" in block:
            tag_count += 1
        out += "\n---\n\n" + block.strip() + "\n"

    write_file(os.path.join(SPECS_DIR, "goals.ad"), out)
    # Verify
    verify = read_file(os.path.join(SPECS_DIR, "goals.ad"))
    verify_tags = verify.count("**Tags:**")
    print(f"  Wrote goals.ad ({len(sorted_ids)} goals, {tag_count} with tags, verified {verify_tags} in file)")
    return sorted_ids


# ── Process Architecture ──────────────────────────────────────────────────────

def process_architecture():
    text = read_file(os.path.join(SPECS_DIR, "architecture.ad"))
    preamble, sections = parse_sections(text, "A")
    first_copy, second_copy = split_at_second_first_id(sections, "A1")

    # Keep second copy A1-A13, keep first copy A3-A6 (unique), discard first A1-A2
    unique_from_first = [s for s in first_copy if s["id"] in ("A3", "A4", "A5", "A6")]

    # Build new IDs directly to avoid dict collisions (both copies have A3-A6)
    second_ids = [f"A{i}" for i in range(1, len(second_copy) + 1)]
    first_ids  = [f"A{i}" for i in range(len(second_copy) + 1, len(second_copy) + len(unique_from_first) + 1)]

    out = preamble.rstrip() + "\n"
    for s, new_id in zip(second_copy, second_ids):
        block = s["header"] + s["body"]
        block = re.sub(rf"^## {re.escape(s['id'])}\b", f"## {new_id}", block, count=1)
        out += "\n---\n\n" + block.strip() + "\n"
    for s, new_id in zip(unique_from_first, first_ids):
        block = s["header"] + s["body"]
        block = re.sub(rf"^## {re.escape(s['id'])}\b", f"## {new_id}", block, count=1)
        out += "\n---\n\n" + block.strip() + "\n"

    total = len(second_copy) + len(unique_from_first)
    write_file(os.path.join(SPECS_DIR, "architecture.ad"), out)
    print(f"  Wrote architecture.ad ({total} items, A1-A{total})")
    # Return flat map for summary only
    return {s["id"]: new_id for s, new_id in zip(second_copy + unique_from_first, second_ids + first_ids)}


# ── Process Designs ───────────────────────────────────────────────────────────

def process_designs():
    text = read_file(os.path.join(SPECS_DIR, "designs.ad"))
    preamble, sections = parse_sections(text, "D")
    first_copy, second_copy = split_at_second_first_id(sections, "D1")

    # Keep second copy (D1-D29), discard first copy
    kept = second_copy

    id_map = {}
    for i, s in enumerate(kept, 1):
        id_map[s["id"]] = f"D{i}"

    out = preamble.rstrip() + "\n"
    for s in kept:
        old_id = s["id"]
        new_id = id_map[old_id]
        block = s["header"] + s["body"]
        block = re.sub(rf"^## {re.escape(old_id)}\b", f"## {new_id}", block, count=1)
        for old_ref, new_ref in sorted(id_map.items(), key=lambda x: len(x[0]), reverse=True):
            block = re.sub(rf"\b{re.escape(old_ref)}\b", new_ref, block)
        out += "\n---\n\n" + block.strip() + "\n"

    write_file(os.path.join(SPECS_DIR, "designs.ad"), out)
    print(f"  Wrote designs.ad ({len(kept)} items, D1-D{len(kept)})")
    return id_map


# ── Process Plans ─────────────────────────────────────────────────────────────

def process_plans():
    text = read_file(os.path.join(SPECS_DIR, "plans.ad"))
    preamble, sections = parse_sections(text, "P")
    first_copy, second_copy = split_at_second_first_id(sections, "P1")

    # Keep second copy (P1-P19), discard first copy
    kept = second_copy

    id_map = {}
    for i, s in enumerate(kept, 1):
        id_map[s["id"]] = f"P{i}"

    out = preamble.rstrip() + "\n"
    for s in kept:
        old_id = s["id"]
        new_id = id_map[old_id]
        block = s["header"] + s["body"]
        block = re.sub(rf"^## {re.escape(old_id)}\b", f"## {new_id}", block, count=1)
        for old_ref, new_ref in sorted(id_map.items(), key=lambda x: len(x[0]), reverse=True):
            block = re.sub(rf"\b{re.escape(old_ref)}\b", new_ref, block)
        out += "\n---\n\n" + block.strip() + "\n"

    write_file(os.path.join(SPECS_DIR, "plans.ad"), out)
    print(f"  Wrote plans.ad ({len(kept)} items, P1-P{len(kept)})")
    return id_map


# ── Process Tests ─────────────────────────────────────────────────────────────

def process_tests():
    text = read_file(os.path.join(SPECS_DIR, "tests.ad"))
    preamble, sections = parse_sections(text, "S")

    if not sections:
        print("  tests.ad: no sections found")
        return {}

    # Check duplicates by title
    seen = {}
    unique = []
    for s in sections:
        if s["title"] not in seen:
            seen[s["title"]] = True
            unique.append(s)

    if len(unique) != len(sections):
        print(f"  Removed {len(sections) - len(unique)} duplicate test sections")
        sections = unique

    id_map = {}
    for i, s in enumerate(sections, 1):
        id_map[s["id"]] = f"S{i}"

    out = preamble.rstrip() + "\n"
    for s in sections:
        old_id = s["id"]
        new_id = id_map[old_id]
        block = s["header"] + s["body"]
        block = re.sub(rf"^## {re.escape(old_id)}\b", f"## {new_id}", block, count=1)
        for old_ref, new_ref in sorted(id_map.items(), key=lambda x: len(x[0]), reverse=True):
            block = re.sub(rf"\b{re.escape(old_ref)}\b", new_ref, block)
        out += "\n---\n\n" + block.strip() + "\n"

    write_file(os.path.join(SPECS_DIR, "tests.ad"), out)
    print(f"  Wrote tests.ad ({len(sections)} items)")
    return id_map


# ── Update Manifest ───────────────────────────────────────────────────────────

def update_manifest(counts):
    path = os.path.join(SPECS_DIR, "manifest.at")
    text = read_file(path)
    text = re.sub(r'version = \d+', f'version = {counts["version"]}', text)
    ts = int(datetime.now(timezone.utc).timestamp())
    text = re.sub(r'last_modified = \d+', f'last_modified = {ts}', text)
    write_file(path, text)
    print(f"  Wrote manifest.at (version {counts['version']})")


# ── Main ──────────────────────────────────────────────────────────────────────

def main():
    print("Regenerating specs...")
    print()

    backups = [d for d in os.listdir(".") if d.startswith("specs-backup-")]
    if backups:
        print(f"Backup found: {backups[-1]}")
    else:
        print("WARNING: No backup found!")

    goal_ids = process_goals()
    arch_map = process_architecture()
    design_map = process_designs()
    plan_map = process_plans()
    test_map = process_tests()

    counts = {
        "version": 22,
        "goals": len(goal_ids),
        "architecture": len(arch_map),
        "designs": len(design_map),
        "plans": len(plan_map),
        "tests": len(test_map),
    }
    update_manifest(counts)

    print()
    print("Done! Summary:")
    for k, v in counts.items():
        if k != "version":
            print(f"  {k.capitalize():12} {v} items")


if __name__ == "__main__":
    main()
