#!/usr/bin/env python3
"""
USFM Documentation Importer

Fetches RST documentation from https://ubsicap.github.io/usfm/ and converts
it to a format suitable for Claude to reference.

Usage:
    python .claude/import_docs.py
"""

import os
import re
import urllib.request
import urllib.error
from pathlib import Path
from typing import Set
from collections import deque

BASE_URL = "https://ubsicap.github.io/usfm/_sources"
OUTPUT_DIR = Path(__file__).parent / "docs"

# Regex to find toctree entries
# Matches lines like: "   About USFM <about/index>" or "   releasenotes"
TOCTREE_ENTRY_RE = re.compile(r'^\s+(?:[^<]+<)?([^<>\s]+)>?\s*$')

# Regex to find doc references like :doc:`Release Notes </about/releasenotes>`
DOC_REF_RE = re.compile(r':doc:`[^`]*<([^>]+)>`|:doc:`([^`]+)`')


def fetch_url(url: str) -> str | None:
    """Fetch content from URL, return None on error."""
    try:
        with urllib.request.urlopen(url, timeout=30) as response:
            return response.read().decode('utf-8')
    except (urllib.error.URLError, urllib.error.HTTPError) as e:
        print(f"  Error fetching {url}: {e}")
        return None


def extract_toctree_entries(content: str) -> list[str]:
    """Extract all file references from toctree directives."""
    entries = []
    in_toctree = False

    for line in content.split('\n'):
        # Check for toctree directive
        if '.. toctree::' in line:
            in_toctree = True
            continue

        # If in toctree, look for entries
        if in_toctree:
            # Empty line or new directive ends toctree
            if line.strip() == '' or (line.strip() and not line.startswith(' ')):
                # But allow blank lines within toctree
                if line.strip() and not line.startswith(' ') and not line.startswith(':'):
                    in_toctree = False
                continue

            # Skip toctree options like :maxdepth:
            if line.strip().startswith(':'):
                continue

            # Extract entry
            match = TOCTREE_ENTRY_RE.match(line)
            if match:
                entry = match.group(1)
                if entry:
                    entries.append(entry)

    return entries


def extract_doc_refs(content: str) -> list[str]:
    """Extract doc references like :doc:`/about/releasenotes`."""
    refs = []
    for match in DOC_REF_RE.finditer(content):
        ref = match.group(1) or match.group(2)
        if ref:
            refs.append(ref)
    return refs


def normalize_path(current_dir: str, ref: str) -> str:
    """Normalize a reference path relative to current directory."""
    # Absolute path
    if ref.startswith('/'):
        return ref[1:]  # Remove leading slash

    # Relative path
    if current_dir:
        return f"{current_dir}/{ref}"
    return ref


def rst_to_markdown(content: str, source_path: str) -> str:
    """Convert RST content to a more readable format for Claude."""
    lines = content.split('\n')
    output = []

    # Add source reference
    output.append(f"<!-- Source: {BASE_URL}/{source_path}.rst.txt -->")
    output.append("")

    i = 0
    while i < len(lines):
        line = lines[i]

        # Skip include directives
        if line.strip().startswith('.. include::'):
            i += 1
            continue

        # Convert RST title underlines to markdown headers
        if i + 1 < len(lines) and lines[i + 1].strip():
            underline = lines[i + 1].strip()
            if len(underline) >= len(line.strip()) and len(set(underline)) == 1:
                char = underline[0]
                if char in '*=-~^"':
                    level = {'*': 1, '=': 2, '-': 3, '~': 4, '^': 5, '"': 6}.get(char, 3)
                    output.append('#' * level + ' ' + line.strip())
                    i += 2
                    continue

        # Convert index directives to comments
        if line.strip().startswith('.. index::'):
            output.append(f"<!-- index: {line.split('::', 1)[1].strip()} -->")
            i += 1
            continue

        # Convert topic directives
        if line.strip().startswith('.. topic::'):
            topic = line.split('::', 1)[1].strip()
            output.append(f"**{topic}**")
            i += 1
            continue

        # Skip toctree directives (we handle them separately)
        if line.strip() == '.. toctree::':
            # Skip until we hit a non-indented line
            i += 1
            while i < len(lines) and (lines[i].startswith(' ') or lines[i].strip() == ''):
                i += 1
            continue

        # Convert code blocks
        if line.strip().startswith('.. code-block::'):
            lang = line.split('::', 1)[1].strip() or 'text'
            output.append(f"```{lang}")
            i += 1
            # Skip blank line after directive
            if i < len(lines) and lines[i].strip() == '':
                i += 1
            # Collect indented code
            while i < len(lines) and (lines[i].startswith('   ') or lines[i].strip() == ''):
                if lines[i].strip():
                    output.append(lines[i][3:])  # Remove 3-space indent
                else:
                    output.append('')
                i += 1
            output.append("```")
            continue

        # Convert RST inline markup
        # ``code`` -> `code`
        line = re.sub(r'``([^`]+)``', r'`\1`', line)
        # **bold** stays the same
        # *italic* stays the same

        # Convert :doc: references to just the title
        line = re.sub(r':doc:`([^<`]+)<[^>]+>`', r'**\1**', line)
        line = re.sub(r':doc:`([^`]+)`', r'**\1**', line)

        # Convert :ref: references
        line = re.sub(r':ref:`([^<`]+)<[^>]+>`', r'*\1*', line)
        line = re.sub(r':ref:`([^`]+)`', r'*\1*', line)

        output.append(line)
        i += 1

    return '\n'.join(output)


def create_index(docs: dict[str, str]) -> str:
    """Create a master index of all documentation."""
    index = ["# USFM Documentation Index", ""]
    index.append("This directory contains the USFM specification documentation.")
    index.append("Use these files as reference when implementing USFM parsing.")
    index.append("")
    index.append("## Files")
    index.append("")

    # Group by directory
    by_dir: dict[str, list[str]] = {}
    for path in sorted(docs.keys()):
        parts = path.split('/')
        if len(parts) > 1:
            dir_name = parts[0]
        else:
            dir_name = "(root)"

        if dir_name not in by_dir:
            by_dir[dir_name] = []
        by_dir[dir_name].append(path)

    for dir_name, paths in sorted(by_dir.items()):
        index.append(f"### {dir_name}")
        for path in paths:
            filename = path.split('/')[-1]
            index.append(f"- [{filename}]({path}.md)")
        index.append("")

    return '\n'.join(index)


def main():
    print("USFM Documentation Importer")
    print("=" * 40)

    # Create output directory
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)

    # Track visited and pending URLs
    visited: Set[str] = set()
    pending: deque[str] = deque(['index'])
    docs: dict[str, str] = {}

    while pending:
        doc_path = pending.popleft()

        # Normalize path
        doc_path = doc_path.lstrip('/')

        if doc_path in visited:
            continue
        visited.add(doc_path)

        # Construct URL
        url = f"{BASE_URL}/{doc_path}.rst.txt"
        print(f"Fetching: {doc_path}")

        content = fetch_url(url)
        if content is None:
            continue

        docs[doc_path] = content

        # Extract links
        current_dir = '/'.join(doc_path.split('/')[:-1])

        # From toctree
        for entry in extract_toctree_entries(content):
            ref_path = normalize_path(current_dir, entry)
            if ref_path not in visited:
                pending.append(ref_path)

        # From doc references
        for ref in extract_doc_refs(content):
            ref_path = normalize_path(current_dir, ref)
            if ref_path not in visited:
                pending.append(ref_path)

    print(f"\nFetched {len(docs)} documents")
    print("\nConverting to Markdown...")

    # Convert and save
    for doc_path, content in docs.items():
        # Create directory structure
        output_path = OUTPUT_DIR / f"{doc_path}.md"
        output_path.parent.mkdir(parents=True, exist_ok=True)

        # Convert and save
        markdown = rst_to_markdown(content, doc_path)
        output_path.write_text(markdown)
        print(f"  Wrote: {output_path.relative_to(OUTPUT_DIR.parent)}")

    # Create index
    index_content = create_index(docs)
    index_path = OUTPUT_DIR / "INDEX.md"
    index_path.write_text(index_content)
    print(f"  Wrote: {index_path.relative_to(OUTPUT_DIR.parent)}")

    print(f"\nDone! Documentation saved to {OUTPUT_DIR}")


if __name__ == "__main__":
    main()
