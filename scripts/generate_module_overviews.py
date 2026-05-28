#!/usr/bin/env python3
"""
Generate enriched module.ad for each module directory.
Extracts structured information from all spec files in the module.
"""

import re
from pathlib import Path

SPEC_DIR = Path("specs/auto-forge")

MODULE_TEMPLATE = """# {module_title} Module

## Overview

{description}

## Goals

{goals_list}

## Key Architecture Decisions

{architecture_list}

## Key Designs

{designs_list}

## Module Dependencies

{dependencies}

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


def extract_section_items(content: str, section_name: str) -> list[dict]:
    """Extract items from a spec file. e.g. ## G1 Title -> {"id": "G1", "title": "Title"}"""
    items = []
    # Match lines like: ## G1 Title here
    # or: ## Relay-G1 Title here
    pattern = re.compile(r'^##\s+(?:(\w+)-)?(\w+\d+)\s+(.*?)$', re.MULTILINE)
    for match in pattern.finditer(content):
        prefix = match.group(1) or ""
        item_id = match.group(2)
        title = match.group(3).strip()
        items.append({"prefix": prefix, "id": item_id, "title": title})
    return items


def extract_decisions(content: str) -> list[dict]:
    """Extract Decision summaries from architecture.ad content."""
    decisions = []
    # Find each Architecture item and extract its Decision line
    items = extract_section_items(content, "architecture")
    for item in items:
        # Try to find the Decision line after this item's header
        header_pattern = re.escape(f"## {item['prefix']}-{item['id']}" if item['prefix'] else f"## {item['id']}") + r'\s+' + re.escape(item['title'])
        match = re.search(header_pattern + r'(.*?)(?=^##|\Z)', content, re.MULTILINE | re.DOTALL)
        if match:
            block = match.group(1)
            decision_match = re.search(r'\*\*Decision:\*\*\s*(.*?)(?:\n\n|\n##|\Z)', block, re.DOTALL)
            if decision_match:
                decision = decision_match.group(1).strip().replace('\n', ' ')
                # Truncate long decisions
                if len(decision) > 120:
                    decision = decision[:117] + "..."
                decisions.append({"id": item["id"], "title": item["title"], "decision": decision})
            else:
                decisions.append({"id": item["id"], "title": item["title"], "decision": ""})
        else:
            decisions.append({"id": item["id"], "title": item["title"], "decision": ""})
    return decisions


def extract_dependencies(module_dir: Path, all_modules: set[str]) -> list[str]:
    """Scan all spec files in module for Depends on references to other modules."""
    deps = set()
    for spec_file in module_dir.glob("*.ad"):
        content = spec_file.read_text(encoding="utf-8")
        # Match "Depends on:" lines and extract module prefixes like "Relay-G1", "Chat-D3"
        depends_matches = re.findall(r'\*\*Depends on:\*\*\s*(.*?)(?:\n|$)', content, re.MULTILINE)
        for match in depends_matches:
            # Extract ModulePrefix-XX references
            refs = re.findall(r'([A-Z][a-zA-Z]*)-[A-Z]\d+', match)
            for ref in refs:
                module_prefix = ref
                # Map prefix back to module name
                for mod in all_modules:
                    expected_prefix = "".join(w.capitalize() for w in mod.split("-"))
                    if expected_prefix == module_prefix and mod != module_dir.name:
                        deps.add(mod)
    return sorted(deps)


def extract_goals_summary(module_dir: Path) -> list[dict]:
    """Extract all goals from goals.ad."""
    goals_file = module_dir / "goals.ad"
    if not goals_file.exists():
        return []
    content = goals_file.read_text(encoding="utf-8")
    return extract_section_items(content, "goals")


def extract_architecture_summary(module_dir: Path) -> list[dict]:
    """Extract architecture items with decisions from architecture.ad."""
    arch_file = module_dir / "architecture.ad"
    if not arch_file.exists():
        return []
    content = arch_file.read_text(encoding="utf-8")
    return extract_decisions(content)


def extract_designs_summary(module_dir: Path) -> list[dict]:
    """Extract all designs from designs.ad."""
    designs_file = module_dir / "designs.ad"
    if not designs_file.exists():
        return []
    content = designs_file.read_text(encoding="utf-8")
    return extract_section_items(content, "designs")


def count_specs(module_dir: Path) -> dict[str, int]:
    """Count spec items per type."""
    type_counts = {}
    for filename in ["goals", "architecture", "designs", "plans", "tests", "reviews", "reports"]:
        filepath = module_dir / f"{filename}.ad"
        if filepath.exists():
            fc = filepath.read_text(encoding="utf-8")
            count = len(re.findall(r'^##\s+', fc, re.MULTILINE))
            type_counts[filename] = count
    return type_counts


def module_name_to_prefix(module_name: str) -> str:
    """agent-config -> AgentConfig"""
    return "".join(w.capitalize() for w in module_name.split("-"))


def generate_module_ad(module_dir: Path, all_modules: set[str]) -> str | None:
    """Generate enriched module.ad content for a module directory."""
    module_name = module_dir.name

    # Extract first goal description for overview
    goals = extract_goals_summary(module_dir)
    if not goals:
        return None

    description = goals[0]["title"] if goals else ""
    prefix = module_name_to_prefix(module_name)
    module_title = prefix

    # Extract architecture decisions
    arch_items = extract_architecture_summary(module_dir)

    # Extract designs
    design_items = extract_designs_summary(module_dir)

    # Extract dependencies
    deps = extract_dependencies(module_dir, all_modules)

    # Count specs
    type_counts = count_specs(module_dir)

    # Build goals list (first 5, then "...and N more")
    goals_lines = []
    for g in goals[:6]:
        goals_lines.append(f"- **{g['id']}** — {g['title']}")
    if len(goals) > 6:
        goals_lines.append(f"- *...and {len(goals) - 6} more*")

    # Build architecture list (first 4 with decisions)
    arch_lines = []
    for a in arch_items[:5]:
        if a["decision"]:
            arch_lines.append(f"- **{a['id']}** — {a['title']}: *{a['decision']}*")
        else:
            arch_lines.append(f"- **{a['id']}** — {a['title']}")
    if len(arch_items) > 5:
        arch_lines.append(f"- *...and {len(arch_items) - 5} more*")
    if not arch_lines:
        arch_lines.append("_No architecture decisions documented yet._")

    # Build designs list (first 6)
    design_lines = []
    for d in design_items[:7]:
        design_lines.append(f"- **{d['id']}** — {d['title']}")
    if len(design_items) > 7:
        design_lines.append(f"- *...and {len(design_items) - 7} more*")
    if not design_lines:
        design_lines.append("_No designs documented yet._")

    # Build dependencies
    if deps:
        dep_links = [f"[{d}]({d}/module.ad)" for d in deps]
        dependencies = "Depends on: " + ", ".join(dep_links)
    else:
        dependencies = "_No external module dependencies documented._"

    # Build spec rows
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

    return MODULE_TEMPLATE.format(
        module_title=module_title,
        description=description,
        goals_list="\n".join(goals_lines) if goals_lines else "_No goals documented._",
        architecture_list="\n".join(arch_lines),
        designs_list="\n".join(design_lines) if design_lines else "_No designs documented._",
        dependencies=dependencies,
        spec_rows="\n".join(spec_rows),
        prefix=prefix,
    )


def main():
    # Collect all module names first (for dependency resolution)
    all_modules = set()
    for module_dir in sorted(SPEC_DIR.iterdir()):
        if module_dir.is_dir() and module_dir.name != ".git":
            all_modules.add(module_dir.name)

    for module_dir in sorted(SPEC_DIR.iterdir()):
        if not module_dir.is_dir() or module_dir.name == ".git":
            continue

        content = generate_module_ad(module_dir, all_modules)
        if content is None:
            print(f"Skipped {module_dir.name} (no goals)")
            continue

        (module_dir / "module.ad").write_text(content, encoding="utf-8")
        print(f"Generated {module_dir.name}/module.ad")


if __name__ == "__main__":
    main()
