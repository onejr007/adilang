"""
adilang/cli.py — Standalone ADILang CLI Tool
=============================================
Command line interface for validating, parsing, and formatting ADILang IR files.

Usage:
  adilang-cli parse <file.adi>
  adilang-cli check <file.adi>
  adilang-cli fix <file.adi>
  adilang-cli version

Lead Developer: BAGAS ADI PRATAMA S,Kom.
"""
import argparse
import sys
import json
from adilang.protocol import VERSION, parse_adilang, validate_adilang, auto_fix


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

    # Command: fix
    fix_cmd = subparsers.add_parser("fix", help="Auto-fix invalid ADILang syntax or keys")
    fix_cmd.add_argument("file", help="Path to .adi file")

    args = parser.parse_args()

    if not args.command:
        parser.print_help()
        sys.exit(1)

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

    elif args.command == "fix":
        fixed = auto_fix(content)
        print(fixed)


if __name__ == "__main__":
    main()
