#!/usr/bin/env bash
set -euo pipefail

for command in git grep; do
  command -v "$command" >/dev/null 2>&1 || { echo "$command is required" >&2; exit 1; }
done

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

private_markers='desti''ny|super''connect|HKUST\(GZ\)|One''VLA|Soul''Link|tencent ''cloud|one''vla_vlm_tag|vla_''dev|/Use''rs/geminihe|/ho''me/geminihe'
generic_homes='/Users/[A-Za-z0-9._-]+|/home/[A-Za-z0-9._-]+'
email_pattern='[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}'
secret_markers='s''k-[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9]{20,}|Bear''er[[:space:]]+[A-Za-z0-9._~-]{20,}'
failed=0

while IFS= read -r commit; do
  matches="$(git grep -n -I -i -E "$private_markers" "$commit" -- . || true)"
  if [[ -n "$matches" ]]; then
    echo "Private markers remain reachable from commit $commit:" >&2
    echo "$matches" >&2
    failed=1
    break
  fi
  matches="$(git grep -n -I -E -o "$generic_homes" "$commit" -- . \
    | grep -Ev '/(Users|home)/(example|test|g)$' || true)"
  if [[ -n "$matches" ]]; then
    echo "Non-synthetic home paths remain reachable from commit $commit:" >&2
    echo "$matches" >&2
    failed=1
    break
  fi
  matches="$(git grep -n -I -i -E -o "$email_pattern" "$commit" -- . \
    | grep -Evi '@(example\.(com|org|net|internal)|github\.com|users\.noreply\.github\.com)$' || true)"
  if [[ -n "$matches" ]]; then
    echo "Non-synthetic email addresses remain reachable from commit $commit:" >&2
    echo "$matches" >&2
    failed=1
    break
  fi
  matches="$(git grep -n -I -E -o "$secret_markers" "$commit" -- . \
    | grep -Ev ':(sk-example|gh[pousr]_example|Bearer[[:space:]]+example)' || true)"
  if [[ -n "$matches" ]]; then
    echo "Credential-like values remain reachable from commit $commit:" >&2
    echo "$matches" >&2
    failed=1
    break
  fi
done < <(git rev-list --all)

legacy_private_assets="$(git rev-list --objects --all | grep -E ' docs/(macos-dashboard\.png|.*private.*\.(png|jpg|jpeg))$' || true)"
if [[ -n "$legacy_private_assets" ]]; then
  echo "Legacy private screenshot assets remain reachable:" >&2
  echo "$legacy_private_assets" >&2
  failed=1
fi

if (( failed != 0 )); then
  exit 1
fi

echo "Reachable Git history privacy scan passed."
