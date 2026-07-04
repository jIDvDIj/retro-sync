#!/usr/bin/env sh
# Verifica as licenças de todas as dependências Rust contra a política em
# src-tauri/deny.toml — o mesmo check que o CI roda em cada PR.
#
# Requer cargo-deny. Se não estiver instalado, rode manualmente:
#   cargo install cargo-deny --locked
set -eu

cd "$(git rev-parse --show-toplevel)"

if ! command -v cargo-deny >/dev/null 2>&1; then
  echo "cargo-deny não encontrado. Instale com:" >&2
  echo "  cargo install cargo-deny --locked" >&2
  exit 1
fi

cargo deny --manifest-path src-tauri/Cargo.toml check licenses
