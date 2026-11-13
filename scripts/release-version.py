#!/usr/bin/env python3

import argparse
from datetime import date
from pathlib import Path
import re
import sys


VERSION_PATTERN = re.compile(r"(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\Z")
MANIFESTS = (
    ("Cargo.toml", "workspace.package"),
    ("firmware/nrf52840/Cargo.toml", "package"),
    ("host/heap-probe/Cargo.toml", "package"),
    ("resetter/Cargo.toml", "package"),
)


def parse_version(value):
    match = VERSION_PATTERN.fullmatch(value)
    if match is None:
        raise ValueError("version must be MAJOR.MINOR.PATCH without leading zeros or suffixes")
    parts = tuple(int(part) for part in match.groups())
    if any(part > 2**64 - 1 for part in parts):
        raise ValueError("version components must fit unsigned 64-bit integers")
    return parts


def next_version(current, bump, explicit="", allow_current=False):
    major, minor, patch = parse_version(current)
    if bump == "set":
        candidate = parse_version(explicit)
    elif explicit:
        raise ValueError("an explicit version is only accepted with bump=set")
    elif bump == "major":
        candidate = (major + 1, 0, 0)
    elif bump == "minor":
        candidate = (major, minor + 1, 0)
    elif bump == "patch":
        candidate = (major, minor, patch + 1)
    else:
        raise ValueError("bump must be patch, minor, major, or set")
    version = ".".join(str(part) for part in candidate)
    parse_version(version)
    if candidate < (major, minor, patch) or (candidate == (major, minor, patch) and not allow_current):
        raise ValueError("new version must be greater than the current version")
    return version


def manifest_version(text, section):
    versions = []
    active = False
    for line in text.splitlines():
        stripped = line.strip()
        if stripped.startswith("["):
            active = stripped == "[" + section + "]"
        elif active:
            match = re.fullmatch(r'version\s*=\s*"([^"\n]*)"\s*', stripped)
            if match:
                versions.append(match.group(1))
    if len(versions) != 1:
        raise ValueError("expected exactly one string version in [" + section + "]")
    version = versions[0]
    parse_version(version)
    return version


def replace_version(text, section, version):
    parse_version(version)
    lines = text.splitlines(keepends=True)
    active = False
    replacements = 0
    for index, line in enumerate(lines):
        stripped = line.strip()
        if stripped.startswith("["):
            active = stripped == "[" + section + "]"
        elif active and re.match(r"\s*version\s*=", line):
            ending = "\r\n" if line.endswith("\r\n") else "\n" if line.endswith("\n") else ""
            lines[index] = 'version = "' + version + '"' + ending
            replacements += 1
    if replacements != 1:
        raise ValueError("expected exactly one version in [" + section + "]")
    updated = "".join(lines)
    if manifest_version(updated, section) != version:
        raise ValueError("manifest version replacement failed")
    return updated


def rotate_changelog(text, version, released_on):
    parse_version(version)
    if date.fromisoformat(released_on).isoformat() != released_on:
        raise ValueError("release date must be YYYY-MM-DD")
    heading = re.search(r"^## \[Unreleased\][ \t]*$", text, re.MULTILINE)
    if heading is None:
        raise ValueError("CHANGELOG.md must contain ## [Unreleased]")
    if re.search(r"^## \[" + re.escape(version) + r"\](?:\s|$)", text, re.MULTILINE):
        raise ValueError("CHANGELOG.md already contains this release version")
    remaining = text[heading.end():]
    next_heading = re.search(r"^## ", remaining, re.MULTILINE)
    end = next_heading.start() if next_heading else len(remaining)
    notes = remaining[:end].strip()
    if not any(line.strip() and not line.lstrip().startswith("#") for line in notes.splitlines()):
        raise ValueError("CHANGELOG.md Unreleased section must contain release notes")
    prefix = text[:heading.end()].rstrip()
    suffix = remaining[end:].lstrip()
    rotated = prefix + "\n\n## [" + version + "] - " + released_on + "\n\n" + notes + "\n"
    if suffix:
        rotated += "\n" + suffix
    return rotated, notes + "\n"


def update(root, bump, explicit="", check=False, allow_current=False, released_on=None, notes_path=None):
    inputs = []
    for filename, section in MANIFESTS:
        manifest = root / filename
        text = manifest.read_text()
        inputs.append((manifest, section, text, manifest_version(text, section)))
    current = inputs[0][3]
    if any(version != current for _, _, _, version in inputs):
        raise ValueError("all workspace, firmware, probe, and resetter versions must match before release")
    version = next_version(current, bump, explicit, allow_current)
    outputs = [(manifest, replace_version(text, section, version)) for manifest, section, text, _ in inputs]
    notes = None
    if released_on is not None:
        changelog = root / "CHANGELOG.md"
        rotated, notes = rotate_changelog(changelog.read_text(), version, released_on)
        outputs.append((changelog, rotated))
    if not check:
        for manifest, text in outputs:
            manifest.write_text(text)
        if notes_path is not None and notes is not None:
            notes_path.write_text(notes)
    return version


def main():
    parser = argparse.ArgumentParser(description="Bump all kb.wlt release manifests together")
    parser.add_argument("--bump", choices=["patch", "minor", "major", "set"], required=True)
    parser.add_argument("--version", default="")
    parser.add_argument("--check", action="store_true")
    parser.add_argument("--allow-current", action="store_true")
    parser.add_argument("--date")
    parser.add_argument("--notes", type=Path)
    parser.add_argument("--root", type=Path, default=Path(__file__).resolve().parents[1])
    args = parser.parse_args()
    try:
        print(update(args.root, args.bump, args.version, args.check, args.allow_current, args.date, args.notes))
    except (KeyError, OSError, ValueError) as error:
        print("error: " + str(error), file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
