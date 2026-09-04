#!/usr/bin/env bash
set -euo pipefail

for command in find git perl sort; do
  command -v "$command" >/dev/null 2>&1 || { echo "$command is required" >&2; exit 1; }
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

failed=0
while IFS= read -r file; do
  while IFS= read -r target; do
    [[ -z "$target" ]] && continue
    case "$target" in
      http://*|https://*|mailto:*|codex://*|\#*) continue ;;
    esac

    target="${target%%#*}"
    target="${target#<}"
    target="${target%>}"
    resolved="$(dirname "$file")/$target"
    if [[ ! -e "$resolved" ]]; then
      echo "$file: missing Markdown target '$target'" >&2
      failed=1
    fi
  done < <(perl -ne 'while (/\]\(([^)]+)\)/g) { print "$1\n" }' "$file")
done < <(git ls-files '*.md'; find . -type f -name '*.md' -not -path './target/*' -not -path './web/node_modules/*' -not -path './web/dist/*' -not -path './dist/*' | sed 's#^./##') | sort -u

if (( failed != 0 )); then
  exit 1
fi

echo "Markdown link check passed."
