#!/usr/bin/env bash
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

cd "$root/apps/web"
npm install --no-audit --no-fund
npm run check

cd "$root"
cargo build --release --locked

printf '\nHOOKTRY binary: %s\n' "$root/target/release/hooktry"
printf 'Run local UI: %s\n' "$root/target/release/hooktry ui"
