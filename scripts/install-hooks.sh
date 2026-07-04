#!/usr/bin/env sh
# Instala os git hooks versionados em scripts/git-hooks/ no .git/hooks/ local.
# Rode uma vez após clonar o repositório:
#   sh scripts/install-hooks.sh
set -eu

repo_root=$(git rev-parse --show-toplevel)
hooks_dir=$(git rev-parse --git-path hooks)
src_dir="$repo_root/scripts/git-hooks"

for hook in "$src_dir"/*; do
  name=$(basename "$hook")
  cp "$hook" "$hooks_dir/$name"
  chmod +x "$hooks_dir/$name"
  echo "Hook instalado: $hooks_dir/$name"
done

echo "Pronto. Commits agora são validados contra Conventional Commits."
