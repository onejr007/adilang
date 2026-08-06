"""
adilang/cli.py — Standalone ADILang CLI Tool
=============================================
Command line interface for validating, parsing, compacting, and discovery of ADILang IR files.

Usage:
  adilang-cli parse <file.adi>
  adilang-cli check <file.adi>
  adilang-cli compact <file.adi>
  adilang-cli fix <file.adi>
  adilang-cli prompt
  adilang-cli card --name "MyAgent" --url "http://localhost:8000"
  adilang-cli version

Lead Developer: BAGAS ADI PRATAMA S,Kom.
"""
import argparse
import sys
import json
from adilang import (
    VERSION,
    parse_adilang,
    validate_adilang,
    auto_fix,
    optimize_src,
    get_adilang_knowledge,
    create_agent_card,
    agent_card_to_json,
)


def main():
    parser = argparse.ArgumentParser(
        prog="adilang-cli",
        description="ADILang Standalone Command Line Tool"
    )
    parser.add_argument("--version", action="version", version=f"adilang-cli v{VERSION}")
    subparsers = parser.add_subparsers(dest="command", help="Sub-commands")

    # Command: parse
    parse_cmd = subparsers.add_parser("parse", help="Parse .adi file or string to JSON IR structure")
    parse_cmd.add_argument("file", help="Path to .adi file or raw string")

    # Command: check
    check_cmd = subparsers.add_parser("check", help="Check/validate .adi file syntax against closed vocabulary")
    check_cmd.add_argument("file", help="Path to .adi file")

    # Command: compact
    compact_cmd = subparsers.add_parser("compact", help="Compact ADILang IR file to slash token usage (up to -47%)")
    compact_cmd.add_argument("file", help="Path to .adi file")

    # Command: fix
    fix_cmd = subparsers.add_parser("fix", help="Auto-fix invalid ADILang syntax or keys")
    fix_cmd.add_argument("file", help="Path to .adi file")

    # Command: prompt
    prompt_cmd = subparsers.add_parser("prompt", help="Print compact LLM System Prompt reference for injecting into AI")

    # Command: card
    card_cmd = subparsers.add_parser("card", help="Generate /.well-known/adilang.json Agent Card")
    card_cmd.add_argument("--name", default="My ADILang Agent", help="Agent name")
    card_cmd.add_argument("--url", default="http://localhost:8000", help="Agent base URL")

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(1)

    if args.command == "prompt":
        print(get_adilang_knowledge(mode="compact"))
        sys.exit(0)

    if args.command == "card":
        card = create_agent_card(
            name=args.name,
            version=VERSION,
            description="Autonomous AI Agent running ADILang Protocol",
            url=args.url
        )
        print(agent_card_to_json(card))
        sys.exit(0)

    content = ""
    if hasattr(args, "file"):
        try:
            with open(args.file, "r", encoding="utf-8") as f:
                content = f.read()
        except FileNotFoundError:
            content = args.file

    if args.command == "parse":
        parsed = parse_adilang(content)
        print(json.dumps(parsed, indent=2, ensure_ascii=False))

    elif args.command == "check":
        errors = validate_adilang(content)
        if not errors:
            print("[OK] ADILang syntax is valid!")
            sys.exit(0)
        else:
            print("[ERROR] Found syntax/key errors:")
            for err in errors:
                print(f"  - {err}")
            sys.exit(1)

    elif args.command == "compact":
        compacted = optimize_src(content)
        print(compacted)

    elif args.command == "fix":
        fixed_text, _ = auto_fix(content)
        print(fixed_text)


if __name__ == "__main__":
    main()
