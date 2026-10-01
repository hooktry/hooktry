#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$root/apps/web"
npm install --no-audit --no-fund
npm run check

cd "$root"
cargo build --release --locked

printf '\nORTYO binary: %s\n' "$root/target/release/ortyo"
printf 'Run local UI: %s\n' "$root/target/release/ortyo ui"
