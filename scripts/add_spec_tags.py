#!/usr/bin/env python3
"""Add stack/module tags to spec sections."""

import re
import os

SPECS_DIR = "specs"

def read_file(path):
    with open(path, "r", encoding="utf-8") as f:
        return f.read()

def write_file(path, content):
    with open(path, "w", encoding="utf-8") as f:
        f.write(content)

def parse_sections(text, prefix):
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

def add_tags_to_section(section_text, tags):
    if not tags:
        return section_text
    tags_line = "**Tags:** " + ", ".join(tags) + "\n"
    if "**Tags:**" in section_text:
        section_text = re.sub(r"\*\*Tags:\*\*.*\n", tags_line, section_text)
        return section_text
    for pat in [r"(\*\*Depends on:\*\*.*?\n)", r"(\*\*Status:\*\*.*?\n)"]:
        m = re.search(pat, section_text)
        if m:
            insert_pos = m.end()
            return section_text[:insert_pos] + tags_line + section_text[insert_pos:]
    return section_text

# ── Tag mappings ──────────────────────────────────────────────────────────────

DESIGN_TAGS = {
    "D1":  ("stack:backend",  "module:agent-config"),
    "D2":  ("stack:backend",  "module:relay"),
    "D3":  ("stack:backend",  "module:relay"),
    "D4":  ("stack:backend",  "module:relay"),
    "D5":  ("stack:backend",  "module:relay"),
    "D6":  ("stack:backend",  "module:relay"),
    "D7":  ("stack:backend",  "module:agent-config"),
    "D8":  ("stack:frontend", "module:chat"),
    "D9":  ("stack:frontend", "module:relay"),
    "D10": ("stack:frontend", "module:specs"),
    "D11": ("stack:frontend", "module:specs"),
    "D12": ("stack:frontend", "module:relay"),
    "D13": ("stack:frontend", "module:relay"),
    "D14": ("stack:frontend", "module:ui-system"),
    "D15": ("stack:frontend", "module:specs"),
    "D16": ("stack:frontend", "module:relay"),
    "D17": ("stack:frontend", "module:chat"),
    "D18": ("stack:frontend", "module:relay"),
    "D19": ("stack:both",     "module:api-sources"),
    "D20": ("stack:backend",  "module:provider"),
    "D21": ("stack:backend",  "module:provider"),
    "D22": ("stack:backend",  "module:cli"),
    "D23": ("stack:backend",  "module:runtime"),
    "D24": ("stack:both",     "module:chat"),
    "D25": ("stack:both",     "module:wiki"),
    "D26": ("stack:frontend", "module:wiki"),
    "D27": ("stack:frontend", "module:editor"),
    "D28": ("stack:both",     "module:errand"),
    "D29": ("stack:backend",  "module:relay"),
    "D30": ("stack:frontend", "module:ui-system"),
    "D31": ("stack:frontend", "module:relay"),
    "D32": ("stack:frontend", "module:chat"),
    "D33": ("stack:frontend", "module:project"),
    "D34": ("stack:frontend", "module:api-sources"),
}

PLAN_TAGS = {
    "P1":  ("stack:both",     "module:relay"),
    "P2":  ("stack:both",     "module:relay"),
    "P3":  ("stack:both",     "module:relay"),
    "P4":  ("stack:both",     "module:relay"),
    "P5":  ("stack:both",     "module:relay"),
    "P6":  ("stack:both",     "module:relay"),
    "P7":  ("stack:both",     "module:relay"),
    "P8":  ("stack:both",     "module:chat"),
    "P9":  ("stack:frontend", "module:relay"),
    "P10": ("stack:frontend", "module:specs"),
    "P11": ("stack:both",     "module:relay"),
    "P12": ("stack:frontend", "module:ui-system"),
    "P13": ("stack:both",     "module:project"),
    "P14": ("stack:both",     "module:api-sources"),
    "P15": ("stack:backend",  "module:provider"),
    "P16": ("stack:both",     "module:chat"),
    "P17": ("stack:both",     "module:wiki"),
    "P18": ("stack:frontend", "module:editor"),
    "P19": ("stack:both",     "module:errand"),
    "P20": ("stack:frontend", "module:ui-system"),
    "P21": ("stack:frontend", "module:relay"),
    "P22": ("stack:frontend", "module:chat"),
    "P23": ("stack:both",     "module:project"),
    "P24": ("stack:frontend", "module:api-sources"),
}

ARCH_TAGS = {
    "A1":  ("stack:backend",  "module:relay"),
    "A2":  ("stack:backend",  "module:agent-config"),
    "A3":  ("stack:backend",  "module:relay"),
    "A4":  ("stack:backend",  "module:relay"),
    "A5":  ("stack:frontend", "module:ui-system"),
    "A6":  ("stack:frontend", "module:chat"),
    "A7":  ("stack:both",     "module:api-sources"),
    "A8":  ("stack:backend",  "module:provider"),
    "A9":  ("stack:both",     "module:chat"),
    "A10": ("stack:both",     "module:wiki"),
    "A11": ("stack:frontend", "module:editor"),
    "A12": ("stack:both",     "module:errand"),
    "A13": ("stack:both",     "module:relay"),
    "A14": ("stack:frontend", "module:specs"),
    "A15": ("stack:backend",  "module:project"),
    "A16": ("stack:frontend", "module:project"),
    "A17": ("stack:frontend", "module:ui-system"),
    "A18": ("stack:frontend", "module:ui-system"),
    "A19": ("stack:frontend", "module:relay"),
    "A20": ("stack:frontend", "module:chat"),
    "A21": ("stack:both",     "module:project"),
    "A22": ("stack:frontend", "module:api-sources"),
}

TEST_TAGS = {
    "S1":  ("stack:backend",  "module:agent-config"),
    "S2":  ("stack:backend",  "module:agent-config"),
    "S3":  ("stack:backend",  "module:relay"),
    "S4":  ("stack:backend",  "module:relay"),
    "S5":  ("stack:backend",  "module:relay"),
    "S6":  ("stack:backend",  "module:relay"),
    "S7":  ("stack:backend",  "module:relay"),
    "S8":  ("stack:backend",  "module:relay"),
    "S9":  ("stack:backend",  "module:relay"),
    "S10": ("stack:backend",  "module:relay"),
    "S11": ("stack:backend",  "module:relay"),
    "S12": ("stack:backend",  "module:relay"),
    "S13": ("stack:backend",  "module:relay"),
    "S14": ("stack:backend",  "module:agent-config"),
    "S15": ("stack:backend",  "module:relay"),
    "S16": ("stack:backend",  "module:relay"),
    "S17": ("stack:backend",  "module:relay"),
    "S18": ("stack:frontend", "module:chat"),
    "S19": ("stack:frontend", "module:chat"),
    "S20": ("stack:frontend", "module:specs"),
    "S21": ("stack:frontend", "module:specs"),
    "S22": ("stack:frontend", "module:specs"),
    "S23": ("stack:frontend", "module:relay"),
    "S24": ("stack:frontend", "module:relay"),
    "S25": ("stack:frontend", "module:relay"),
    "S26": ("stack:frontend", "module:relay"),
    "S27": ("stack:frontend", "module:ui-system"),
    "S28": ("stack:frontend", "module:ui-system"),
    "S29": ("stack:frontend", "module:ui-system"),
    "S30": ("stack:frontend", "module:ui-system"),
    "S31": ("stack:both",     "module:project"),
    "S32": ("stack:both",     "module:api-sources"),
    "S33": ("stack:both",     "module:api-sources"),
    "S34": ("stack:both",     "module:api-sources"),
    "S35": ("stack:backend",  "module:provider"),
    "S36": ("stack:backend",  "module:provider"),
    "S37": ("stack:backend",  "module:provider"),
    "S38": ("stack:backend",  "module:provider"),
    "S39": ("stack:backend",  "module:runtime"),
    "S40": ("stack:backend",  "module:runtime"),
    "S41": ("stack:backend",  "module:runtime"),
    "S42": ("stack:backend",  "module:cli"),
    "S43": ("stack:both",     "module:chat"),
    "S44": ("stack:frontend", "module:chat"),
    "S45": ("stack:backend",  "module:chat"),
    "S46": ("stack:backend",  "module:chat"),
    "S47": ("stack:frontend", "module:chat"),
    "S48": ("stack:both",     "module:wiki"),
    "S49": ("stack:both",     "module:wiki"),
    "S50": ("stack:backend",  "module:wiki"),
    "S51": ("stack:frontend", "module:wiki"),
    "S52": ("stack:frontend", "module:wiki"),
    "S53": ("stack:frontend", "module:wiki"),
    "S54": ("stack:both",     "module:wiki"),
    "S55": ("stack:both",     "module:wiki"),
    "S56": ("stack:frontend", "module:wiki"),
    "S57": ("stack:frontend", "module:wiki"),
}

def process_file(filename, prefix, tag_map):
    path = os.path.join(SPECS_DIR, filename)
    text = read_file(path)
    preamble, sections = parse_sections(text, prefix)
    
    out = preamble.rstrip() + "\n"
    tagged = 0
    for s in sections:
        block = s["header"] + s["body"]
        tags = tag_map.get(s["id"])
        if tags:
            block = add_tags_to_section(block, tags)
            tagged += 1
        out += "\n---\n\n" + block.strip() + "\n"
    
    write_file(path, out)
    print(f"  {filename}: tagged {tagged}/{len(sections)} sections")

def main():
    print("Adding tags to specs...")
    process_file("designs.ad", "D", DESIGN_TAGS)
    process_file("plans.ad", "P", PLAN_TAGS)
    process_file("architecture.ad", "A", ARCH_TAGS)
    process_file("tests.ad", "S", TEST_TAGS)
    print("Done!")

if __name__ == "__main__":
    main()
