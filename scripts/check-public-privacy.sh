#!/usr/bin/env bash
set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"

failed=0

search_public_text() {
  local case_mode="$1"
  local pattern="$2"
  local git_args=(-n -I -E --untracked)
  [[ "$case_mode" == "insensitive" ]] && git_args+=(-i)
  git grep "${git_args[@]}" "$pattern" -- . || true
}

search_public_matches() {
  local case_mode="$1"
  local pattern="$2"
  local git_args=(-n -I -E -o --untracked)
  [[ "$case_mode" == "insensitive" ]] && git_args+=(-i)
  git grep "${git_args[@]}" "$pattern" -- . || true
}

# Split the literals so this guard can scan itself without self-matching.
private_markers='desti''ny|super''connect|HKUST\(GZ\)|One''VLA|Soul''Link|tencent ''cloud|one''vla_vlm_tag|vla_''dev'
if matches="$(search_public_text insensitive "$private_markers")" && [[ -n "$matches" ]]; then
  echo "Private project markers found in public files:" >&2
  echo "$matches" >&2
  failed=1
fi

private_homes='/Use''rs/geminihe|/ho''me/geminihe'
if matches="$(search_public_text sensitive "$private_homes")" && [[ -n "$matches" ]]; then
  echo "Private home paths found in public files:" >&2
  echo "$matches" >&2
  failed=1
fi

generic_homes='/Users/[A-Za-z0-9._-]+|/home/[A-Za-z0-9._-]+'
if matches="$(search_public_matches sensitive "$generic_homes" | grep -Ev '/(Users|home)/(example|test|g)$' || true)" && [[ -n "$matches" ]]; then
  echo "Non-synthetic home paths found in public files:" >&2
  echo "$matches" >&2
  failed=1
fi

email_pattern='[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}'
if matches="$(search_public_matches insensitive "$email_pattern" | grep -Evi '@(example\.(com|org|net|internal)|github\.com|users\.noreply\.github\.com)$' || true)" && [[ -n "$matches" ]]; then
  echo "Non-synthetic email addresses found in public files:" >&2
  echo "$matches" >&2
  failed=1
fi

secret_markers='s''k-[A-Za-z0-9_-]{16,}|gh[pousr]_[A-Za-z0-9]{20,}|Bear''er[[:space:]]+[A-Za-z0-9._~-]{20,}'
if matches="$(search_public_matches sensitive "$secret_markers" | grep -Ev ':(sk-example|gh[pousr]_example|Bearer[[:space:]]+example)' || true)" && [[ -n "$matches" ]]; then
  echo "Credential-like values found in public files:" >&2
  echo "$matches" >&2
  failed=1
fi

if matches="$(search_public_text sensitive 'Account[[:space:]]+[0-9a-fA-F]{8,}')" && [[ -n "$matches" ]]; then
  echo "Raw-style account identifiers found in public files:" >&2
  echo "$matches" >&2
  failed=1
fi

if (( failed != 0 )); then
  exit 1
fi

echo "Public privacy scan passed. Binary assets still require full-resolution review."
