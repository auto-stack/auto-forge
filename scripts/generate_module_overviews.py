#!/usr/bin/env python3
"""
Generate module.ad for each module directory.
Content is extracted from the module's first goal.
"""

import os
import re
from pathlib import Path

SPEC_DIR = Path("specs/auto-forge")

MODULE_TEMPLATE = """# {module_title} Module

## Overview

{description}

## Module Specs

| Type | Count | File |
|------|-------|------|
{spec_rows}

## Navigation

This module follows the standard spec pyramid:
- **Goals** define what the module aims to achieve
- **Architecture** describes high-level design and component interactions
- **Designs** contain detailed technical specifications
- **Plans** track implementation tasks
- **Tests** list acceptance criteria
- **Reviews** capture post-implementation retrospectives
- **Reports** contain implementation summaries

## ID Prefix

Spec IDs in this module use the prefix **`{prefix}-`** (e.g. `{prefix}-G1`, `{prefix}-D1`).
"""


def extract_module_info(module_dir: Path):
    goals_file = module_dir / "goals.ad"
    if not goals_file.exists():
        return None, None, {}

    content = goals_file.read_text(encoding="utf-8")
    # First goal title
    first_goal = re.search(r'^##\s+\S+\s+(.*?)$', content, re.MULTILINE)
    description = first_goal.group(1).strip() if first_goal else ""

    # Count specs per type
    type_counts = {}
    for filename in ["goals", "architecture", "designs", "plans", "tests", "reviews", "reports"]:
        filepath = module_dir / f"{filename}.ad"
        if filepath.exists():
            fc = filepath.read_text(encoding="utf-8")
            count = len(re.findall(r'^##\s+', fc, re.MULTILINE))
            type_counts[filename] = count

    return description, first_goal.group(0) if first_goal else "", type_counts


def module_name_to_prefix(module_name: str) -> str:
    """agent-config -> AgentConfig"""
    return "".join(w.capitalize() for w in module_name.split("-"))


def main():
    for module_dir in sorted(SPEC_DIR.iterdir()):
        if not module_dir.is_dir() or module_dir.name == ".git":
            continue

        module_name = module_dir.name
        description, _, type_counts = extract_module_info(module_dir)
        if not description:
            continue

        prefix = module_name_to_prefix(module_name)
        module_title = prefix

        spec_rows = []
        type_labels = {
            "goals": "Goals",
            "architecture": "Architecture",
            "designs": "Designs",
            "plans": "Plans",
            "tests": "Tests",
            "reviews": "Reviews",
            "reports": "Reports",
        }
        for type_id, label in type_labels.items():
            count = type_counts.get(type_id, 0)
            if count > 0:
                spec_rows.append(f"| {label} | {count} | [{type_id}.ad]({type_id}.ad) |")

        content = MODULE_TEMPLATE.format(
            module_title=module_title,
            description=description,
            spec_rows="\n".join(spec_rows),
            prefix=prefix,
        )

        (module_dir / "module.ad").write_text(content, encoding="utf-8")
        print(f"Generated {module_name}/module.ad")


if __name__ == "__main__":
    main()
