#!/usr/bin/env bash
# Load the demo company and data into a locally installed Frappe Books Flatpak
# (local mode). Re-running replaces nothing automatically — reset the app data
# first for a clean demo:
#
#   rm -rf ~/.var/app/io.frappe.Books/data/io.frappe.Books
#
# then run this script.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SEED="$HERE/demo-data.py"

flatpak run \
  --filesystem="$SEED:ro" \
  --env=BOOKS_SEED="$SEED" \
  --command=sh io.frappe.Books -c '
    set -e
    WS=$XDG_DATA_HOME/io.frappe.Books/bench
    if [ ! -d "$WS/sites" ]; then
      mkdir -p "$WS"
      for e in apps env patches.txt Procfile; do ln -s "/app/books/bench/$e" "$WS/$e"; done
      cp -a /app/books/bench/sites "$WS/sites"
      mkdir -p "$WS/logs" "$WS/config/pids" "$WS/redis"
      cp -a /app/books/bench/config/. "$WS/config/"
    fi
    /app/bin/redis-server --port 11000 --bind 127.0.0.1 --dir "$WS/redis" --save "" --appendonly no --logfile "$WS/redis/q.log" &
    /app/bin/redis-server --port 13000 --bind 127.0.0.1 --dir "$WS/redis" --save "" --appendonly no --logfile "$WS/redis/c.log" &
    sleep 1
    cd "$WS"
    /app/bin/bench --site site1 console < "$BOOKS_SEED"
    pkill -f redis-server 2>/dev/null || true
  '
