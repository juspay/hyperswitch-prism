#!/usr/bin/env python3
"""Reject construction of shared Money inside the connector integration layer."""

from pathlib import Path
import re
import sys


ROOT = Path(__file__).resolve().parents[2]
L3 = ROOT / "crates/integrations/connector-integration/src"
MONEY_USE = re.compile(r"\bMoney\s*(?:\{|::\s*[A-Za-z_]\w*\s*\()")
MONEY_ALIAS = re.compile(r"\bMoney\s+as\s+([A-Za-z_]\w*)")
LOCAL_MONEY = re.compile(r"\b(?:struct|enum|union)\s+Money\b")
QUALIFIED_MONEY = re.compile(r"common_utils\s*::\s*types\s*::\s*$")
RAW_STRING = re.compile(r'(?:b?r)(#*)"')


def code_only(source: str) -> str:
    """Blank comments and strings while retaining offsets and line numbers."""
    output = list(source)
    index = 0

    def blank(start: int, end: int) -> None:
        for position in range(start, end):
            if output[position] != "\n":
                output[position] = " "

    while index < len(source):
        if source.startswith("//", index):
            end = source.find("\n", index)
            if end < 0:
                end = len(source)
            blank(index, end)
            index = end
        elif source.startswith("/*", index):
            start = index
            depth = 1
            index += 2
            while index < len(source) and depth:
                if source.startswith("/*", index):
                    depth += 1
                    index += 2
                elif source.startswith("*/", index):
                    depth -= 1
                    index += 2
                else:
                    index += 1
            blank(start, index)
        else:
            raw = RAW_STRING.match(source, index)
            if raw:
                start = index
                delimiter = '"' + raw.group(1)
                index = raw.end()
                end = source.find(delimiter, index)
                index = len(source) if end < 0 else end + len(delimiter)
                blank(start, index)
            elif source[index] == '"' or source.startswith('b"', index):
                start = index
                index += 2 if source.startswith('b"', index) else 1
                while index < len(source):
                    if source[index] == "\\":
                        index += 2
                    elif source[index] == '"':
                        index += 1
                        break
                    else:
                        index += 1
                blank(start, index)
            else:
                index += 1

    return "".join(output)


def violations(source: str) -> list[int]:
    code = code_only(source)
    local_money = bool(LOCAL_MONEY.search(code))
    aliases = MONEY_ALIAS.findall(code)
    construction = re.compile(
        rf"\b(?:Money|{'|'.join(map(re.escape, aliases))})\s*(?:\{{|::\s*[A-Za-z_]\w*\s*\()"
        if aliases
        else MONEY_USE.pattern
    )
    lines = []
    for match in construction.finditer(code):
        prefix = code[max(0, match.start() - 80) : match.start()]
        qualified = bool(QUALIFIED_MONEY.search(prefix))
        if local_money and match.group().startswith("Money") and not qualified:
            continue
        if re.search(r"\b(?:struct|enum|union)\s+$", prefix):
            continue
        lines.append(source.count("\n", 0, match.start()) + 1)
    return lines


def main() -> int:
    count = 0
    for path in sorted(L3.rglob("*.rs")):
        for line in violations(path.read_text(encoding="utf-8")):
            relative = path.relative_to(ROOT)
            print(f"{relative}:{line}: L3 must not construct domain Money; pass domain Money to the converter or move scalar-to-Money conversion to the domain boundary")
            count += 1
    if count:
        print(f"Found {count} L3 Money constructions", file=sys.stderr)
        return 1
    print("No L3 Money constructions found")
    return 0


if __name__ == "__main__":
    sys.exit(main())
