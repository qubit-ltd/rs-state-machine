#!/usr/bin/env python3
"""Compile the Rust examples against the minimal dependency profiles in the docs."""
from __future__ import annotations

import argparse
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import tomllib


ROOT = Path(__file__).resolve().parents[1]
CONFIG = ROOT / "scripts" / "doc-consumers.json"
FENCE = re.compile(r"^```([^\n]*)\n(.*?)^```[ \t]*$", re.M | re.S)
HEADING = re.compile(r"^(#{1,6})\s+(.+?)\s*#*\s*$", re.M)


class CheckError(RuntimeError):
    pass


def section_text(markdown: str, heading: str) -> str:
    matches = []
    offset = 0
    fenced = False
    for line in markdown.splitlines(keepends=True):
        if line.startswith("```"):
            fenced = not fenced
        elif not fenced:
            match = HEADING.match(line.rstrip("\r\n"))
            if match and match.group(2) == heading:
                matches.append((offset, offset + len(line), len(match.group(1))))
        offset += len(line)
    if len(matches) != 1:
        raise CheckError(f"heading {heading!r}: expected exactly one occurrence, found {len(matches)}")
    start, after_heading, level = matches[0]
    end = len(markdown)
    offset = 0
    fenced = False
    for line in markdown.splitlines(keepends=True):
        if line.startswith("```"):
            fenced = not fenced
        elif offset > after_heading and not fenced:
            match = HEADING.match(line.rstrip("\r\n"))
            if match and len(match.group(1)) <= level:
                end = offset
                break
        offset += len(line)
    return markdown[after_heading:end]


def code_block(markdown: str, heading: str, language: str, ordinal: int) -> tuple[str, str]:
    if ordinal < 1:
        raise CheckError("block ordinals are one-based")
    section = section_text(markdown, heading)
    blocks = [(m.group(1).strip(), m.group(2)) for m in FENCE.finditer(section)]
    selected = [(" ".join(attrs.split()[1:]), body) for attrs, body in blocks if attrs.split()[:1] == [language]]
    if len(selected) < ordinal:
        raise CheckError(f"section {heading!r}: expected {language} block #{ordinal}, found {len(selected)}")
    attrs, body = selected[ordinal - 1]
    return attrs, body


def parse_config(raw: str) -> dict:
    try:
        data = json.loads(raw)
    except json.JSONDecodeError as exc:
        raise CheckError(f"invalid JSON config: {exc}") from exc
    scenarios = data.get("scenarios")
    if not isinstance(scenarios, list) or not scenarios:
        raise CheckError("scenario manifest must be a non-empty list")
    if data.get("expected_scenarios") != len(scenarios):
        raise CheckError(f"scenario count changed: expected {data.get('expected_scenarios')!r}, found {len(scenarios)}")
    names = [item.get("name") for item in scenarios]
    if any(not name for name in names) or len(set(names)) != len(names):
        raise CheckError("scenario names must be present and unique")
    rust_selectors = [
        (item.get("document"), item.get("rust_section"), item.get("rust_block"))
        for item in scenarios
    ]
    if len(set(rust_selectors)) != len(rust_selectors):
        raise CheckError("Rust example block selectors must be unique")
    for item in scenarios:
        if item.get("profile") not in {"standard", "fast"}:
            raise CheckError(f"{item.get('name')}: invalid profile {item.get('profile')!r}")
        for key in ("document", "toml_section", "rust_section"):
            if not item.get(key):
                raise CheckError(f"{item.get('name')}: missing {key}")
        for key in ("toml_block", "rust_block"):
            if not isinstance(item.get(key), int) or item[key] < 1:
                raise CheckError(f"{item.get('name')}: invalid {key}")
    return data


def toml_quote(value: str) -> str:
    return json.dumps(value)


def manifest_text(source: str, item: dict, project: Path) -> str:
    try:
        config = tomllib.loads(source)
    except tomllib.TOMLDecodeError as exc:
        raise CheckError(f"{item['name']}: invalid TOML: {exc}") from exc
    deps = config.get("dependencies")
    if not isinstance(deps, dict):
        raise CheckError(f"{item['name']}: TOML block has no [dependencies]")
    pkg = deps.get("qubit-state-machine")
    if not isinstance(pkg, dict) or pkg.get("default-features") is not False or pkg.get("features") != [item["profile"]]:
        raise CheckError(f"{item['name']}: declared features do not match {item['profile']} profile")
    lines = ['[package]', 'name = "doc-consumer"', 'version = "0.0.0"', 'edition = "2024"', '', '[dependencies]']
    for name, value in deps.items():
        if name == "qubit-state-machine":
            lines.extend([f'{toml_quote(name)} = {{ path = {toml_quote(str(project))}, default-features = false, features = [{toml_quote(item["profile"])}] }}'])
        elif isinstance(value, str):
            lines.append(f'{toml_quote(name)} = {toml_quote(value)}')
        elif isinstance(value, dict):
            attrs = []
            for key, val in value.items():
                if isinstance(val, bool):
                    encoded = str(val).lower()
                elif isinstance(val, list):
                    encoded = "[" + ", ".join(toml_quote(str(v)) for v in val) + "]"
                else:
                    encoded = toml_quote(str(val))
                attrs.append(f"{key} = {encoded}")
            lines.append(f'{toml_quote(name)} = {{ {", ".join(attrs)} }}')
        else:
            raise CheckError(f"{item['name']}: unsupported dependency declaration for {name}")
    return "\n".join(lines) + "\n"


def run_check(root: Path = ROOT, config_path: Path = CONFIG, runner=subprocess.run) -> int:
    config = parse_config(config_path.read_text(encoding="utf-8"))
    extracted = []
    for item in config["scenarios"]:
        doc = (root / item["document"]).read_text(encoding="utf-8")
        _, toml_src = code_block(doc, item["toml_section"], "toml", item["toml_block"])
        attrs, rust_src = code_block(doc, item["rust_section"], "rust", item["rust_block"])
        if attrs:
            raise CheckError(f"{item['name']}: unsupported Rust fence attributes: {attrs}")
        extracted.append((item, toml_src, rust_src))
    with tempfile.TemporaryDirectory(prefix="rsm-doc-consumers-") as tmp:
        temp_root = Path(tmp)
        for index, (item, toml_src, rust_src) in enumerate(extracted):
            consumer = temp_root / f"consumer-{index:02d}"
            (consumer / "src").mkdir(parents=True)
            manifest = manifest_text(toml_src, item, root.resolve())
            (consumer / "Cargo.toml").write_text(manifest, encoding="utf-8")
            (consumer / "src" / "lib.rs").write_text(
                "#![doc = r####\"Extracted from " + item["document"] + "\n\n```\n" + rust_src + "```\n\"####]\n",
                encoding="utf-8",
            )
            command = ["cargo", "test", "--doc", "--manifest-path", str(consumer / "Cargo.toml"), "--target-dir", str(temp_root / "target")]
            print(
                f"{item['document']} [{item['profile']}] {item['name']}: "
                f"TOML {item['toml_section']!r} block #{item['toml_block']}; "
                f"Rust {item['rust_section']!r} block #{item['rust_block']}: {command}",
                flush=True,
            )
            result = runner(command, cwd=root, check=False, capture_output=True, text=True)
            output = (result.stdout or "") + (result.stderr or "")
            if output:
                print(output, end="" if output.endswith("\n") else "\n", flush=True)
            print(f"{item['name']}: exit {result.returncode}", flush=True)
            if result.returncode:
                if any(marker in output.lower() for marker in ("failed to download", "could not resolve", "connection timed out", "network is unreachable", "failed to fetch")):
                    print(f"{item['name']}: Cargo dependency/network failure; environment problem, not evidence of a documentation error", file=sys.stderr, flush=True)
                return result.returncode
    return 0


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--root", type=Path, default=ROOT)
    parser.add_argument("--config", type=Path, default=CONFIG)
    args = parser.parse_args()
    try:
        return run_check(args.root.resolve(), args.config.resolve())
    except (CheckError, OSError) as exc:
        print(f"doc-consumer check failed: {exc}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
