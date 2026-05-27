#!/usr/bin/env python3
"""
Migrate specs from flat type-based organization to module-based organization.

Before: specs/auto-forge/goals.ad, architecture.ad, designs.ad, ... (flat)
After:  specs/auto-forge/relay/goals.ad, chat/goals.ad, wiki/goals.ad, ... (module-based)

ID format: ModulePrefix-TypePrefix-Number  (e.g., Wiki-G1, Relay-D1, Chat-V1)
"""

import os
import re
import shutil
from pathlib import Path

SPEC_DIR = Path("specs/auto-forge")
BACKUP_DIR = Path("specs/auto-forge.bak")

TYPE_MAP = {
    "goals": ("G", "Goal"),
    "architecture": ("A", "Architecture"),
    "designs": ("D", "Design"),
    "plans": ("P", "Plan"),
    "tests": ("T", "Test"),
    "reviews": ("V", "Review"),
    "reports": ("R", "Report"),
}

# Reverse: prefix -> type filename
PREFIX_TO_TYPE = {v[0]: k for k, v in TYPE_MAP.items()}


def capitalize_module(name: str) -> str:
    """ui-system -> UiSystem"""
    return "".join(w.capitalize() for w in name.split("-"))


def parse_specs():
    """
    Parse all specs and build:
    - all_items: dict[old_id] -> {type_prefix, module, title, body, depends, tags, source_file}
    - sub_items: dict[old_id] -> {parent_id, module, title, source_file}
    """
    all_items = {}
    sub_items = {}

    for filename, (prefix, typename) in TYPE_MAP.items():
        filepath = SPEC_DIR / f"{filename}.ad"
        if not filepath.exists():
            continue
        content = filepath.read_text(encoding="utf-8")

        # Pattern for main spec entries
        # Match ## ID Title followed by body until next ## ID or end
        # ID can be: G1, G10.1, TC-RELAY-001, S1, etc.
        pattern = r'^##\s+([A-Z]+[-]?[A-Z]*\d+(?:\.\d+)?)\s+(.*?)$\n(.*?)(?=^##\s+(?:[A-Z]+[-]?[A-Z]*\d+(?:\.\d+)?)|\Z)'
        matches = list(re.finditer(pattern, content, re.MULTILINE | re.DOTALL))

        for m in matches:
            old_id = m.group(1)
            title = m.group(2).strip()
            body = m.group(3).strip()

            # Extract module from tags
            mod_match = re.search(r'\*\*Tags:\*\*.*module:([\w-]+)', body)
            module = mod_match.group(1) if mod_match else 'uncategorized'

            # Extract depends
            dep_match = re.search(r'\*\*Depends on:\*\*(.*?)(?:\n|$)', body)
            depends = []
            if dep_match:
                depends = re.findall(r'[A-Z]+\d+(?:\.\d+)?', dep_match.group(1))

            all_items[old_id] = {
                'type_prefix': prefix,
                'type_name': typename,
                'module': module,
                'title': title,
                'body': body,
                'depends': depends,
                'source_file': filename,
            }

            # Extract sub-items from tables (e.g., P7.1, P16.5)
            table_ids = re.findall(r'^\|\s*([A-Z]\d+\.\d+)\s*\|', body, re.MULTILINE)
            for sub_id in table_ids:
                sub_items[sub_id] = {
                    'parent_id': old_id,
                    'module': module,
                    'source_file': filename,
                }

    # Handle special test cases: TC-RELAY-001
    tests_content = (SPEC_DIR / "tests.ad").read_text(encoding="utf-8")
    tc_pattern = r'^##\s+(TC-[A-Z]+-\d+):\s+(.*?)$\n(.*?)(?=^##\s+(?:TC-[A-Z]+-\d+|[A-Z]+\d+)|\Z)'
    for m in re.finditer(tc_pattern, tests_content, re.MULTILINE | re.DOTALL):
        old_id = m.group(1)
        title = m.group(2).strip()
        body = m.group(3).strip()
        mod_match = re.search(r'TC-([A-Z]+)-\d+', old_id)
        module = mod_match.group(1).lower() if mod_match else 'uncategorized'
        all_items[old_id] = {
            'type_prefix': 'T',
            'type_name': 'Test',
            'module': module,
            'title': title,
            'body': body,
            'depends': [],
            'source_file': 'tests',
        }

    return all_items, sub_items


def build_id_mapping(all_items, sub_items):
    """Build old_id -> new_id mapping."""
    module_counter = {}
    id_mapping = {}

    def natural_sort_key(k):
        return [int(x) if x.isdigit() else x for x in re.split(r'(\d+)', k)]

    # First pass: main items
    for old_id in sorted(all_items.keys(), key=natural_sort_key):
        info = all_items[old_id]
        mod = info['module']
        prefix = info['type_prefix']
        key = (mod, prefix)
        if key not in module_counter:
            module_counter[key] = 1
        num = module_counter[key]
        module_counter[key] += 1
        mod_cap = capitalize_module(mod)
        new_id = f"{mod_cap}-{prefix}{num}"
        id_mapping[old_id] = new_id
        info['new_id'] = new_id
        info['new_num'] = num

    # Second pass: sub-items (P7.1, P16.5, etc.)
    for old_id, info in sub_items.items():
        parent_id = info['parent_id']
        if parent_id in id_mapping:
            parent_new = id_mapping[parent_id]
            mod_prefix = parent_new.rsplit('-', 1)[0]
            new_id = f"{mod_prefix}-{old_id}"
            id_mapping[old_id] = new_id
            info['new_id'] = new_id

    return id_mapping


def rewrite_id_in_text(text, id_mapping):
    """Replace all old IDs in text with new IDs."""
    # Sort by length descending to avoid partial replacements
    sorted_ids = sorted(id_mapping.keys(), key=len, reverse=True)
    
    # Build a regex pattern that matches any old ID as a standalone token
    # Use negative lookbehind/ahead to ensure it's not part of a larger identifier
    # Allowed chars before/after: whitespace, punctuation, start/end of line
    # NOT allowed: word chars or '-' (to prevent matching G1 inside Project-G1)
    
    def make_pattern(ids):
        escaped = [re.escape(k) for k in ids]
        return re.compile(
            r'(?<![A-Za-z0-9.])(' + '|'.join(escaped) + r')(?![A-Za-z0-9.])'
        )
    
    # Step 1: Replace headers ## OldId
    header_pattern = re.compile(
        r'^(##\s+)(' + '|'.join(re.escape(k) for k in sorted_ids) + r')(?=\s)',
        re.MULTILINE
    )
    def header_repl(m):
        return m.group(1) + id_mapping[m.group(2)]
    text = header_pattern.sub(header_repl, text)
    
    # Step 2: Replace body references (not in headers)
    # Use a function that walks through the text and replaces only standalone IDs
    body_pattern = make_pattern(sorted_ids)
    
    def body_repl(m):
        return id_mapping[m.group(1)]
    
    # Split by lines, skip header lines
    lines = text.split('\n')
    result_lines = []
    for line in lines:
        if line.startswith('##'):
            # Already handled in step 1
            result_lines.append(line)
        else:
            result_lines.append(body_pattern.sub(body_repl, line))
    
    return '\n'.join(result_lines)


def rewrite_tags(text):
    """Remove module:xxx tags, keep others."""
    lines = text.split('\n')
    result = []
    for line in lines:
        if '**Tags:**' in line:
            tag_part = line.split('**Tags:**')[1]
            tags = [t.strip() for t in tag_part.split(',')]
            new_tags = [t for t in tags if not t.startswith('module:')]
            if new_tags:
                result.append(f"**Tags:** {', '.join(new_tags)}")
        else:
            result.append(line)
    return '\n'.join(result)


def split_by_module(all_items, id_mapping):
    """Group specs by module and type, return dict[module][type] -> list of spec bodies."""
    module_specs = {}

    for old_id, info in all_items.items():
        mod = info['module']
        type_file = info['source_file']
        type_prefix = info['type_prefix']

        if mod not in module_specs:
            module_specs[mod] = {}
        if type_file not in module_specs[mod]:
            module_specs[mod][type_file] = []

        body = info['body']
        title = info['title']
        new_id = info['new_id']

        # Rewrite IDs in the body
        body = rewrite_id_in_text(body, id_mapping)
        # Rewrite tags (remove module:xxx)
        body = rewrite_tags(body)

        # Construct the spec entry
        spec_text = f"## {new_id} {title}\n{body}\n"
        module_specs[mod][type_file].append((info['new_num'], spec_text))

    return module_specs


def write_module_files(module_specs):
    """Write specs into per-module directories."""
    # Clear existing module directories
    for item in SPEC_DIR.iterdir():
        if item.is_dir() and item.name != '.git':
            shutil.rmtree(item)

    for mod, type_files in module_specs.items():
        mod_dir = SPEC_DIR / mod
        mod_dir.mkdir(parents=True, exist_ok=True)

        for type_file, specs in type_files.items():
            # Sort by number
            specs.sort(key=lambda x: x[0])
            type_name = TYPE_MAP[type_file][1]
            header = f"# {type_name}s\n\n"
            content = header + "---\n\n".join(s[1] for s in specs)
            (mod_dir / f"{type_file}.ad").write_text(content, encoding="utf-8")


def write_manifest(module_specs):
    """Write new manifest.at in project root."""
    manifest_path = SPEC_DIR / "manifest.at"
    lines = ["project auto-forge", ""]
    for mod in sorted(module_specs.keys()):
        lines.append(f"module {mod}")
    lines.append("")
    manifest_path.write_text("\n".join(lines), encoding="utf-8")


def main():
    print("Parsing specs...")
    all_items, sub_items = parse_specs()
    print(f"Found {len(all_items)} main specs + {len(sub_items)} sub-items")

    print("Building ID mapping...")
    id_mapping = build_id_mapping(all_items, sub_items)

    print(f"\nID mapping sample:")
    for old_id in sorted(list(id_mapping.keys()))[:10]:
        print(f"  {old_id} -> {id_mapping[old_id]}")

    print("\nSplitting by module...")
    module_specs = split_by_module(all_items, id_mapping)

    print(f"Modules: {sorted(module_specs.keys())}")
    for mod, types in sorted(module_specs.items()):
        total = sum(len(v) for v in types.values())
        detail = ", ".join(f"{k}:{len(v)}" for k, v in sorted(types.items()))
        print(f"  {mod:20s} total={total:3d} ({detail})")

    print("\nWriting module files...")
    write_module_files(module_specs)

    print("Writing manifest.at...")
    write_manifest(module_specs)

    print("\nDone! Review the output in specs/auto-forge/")
    print("Backup available at specs/auto-forge.bak/")


if __name__ == "__main__":
    main()
