#!/bin/sh
# aion#32 live flatness instrument (Vesper) — one row every 180s.
# Reads the serving pid fresh each row (survives a server cycle); a dead or
# missing pid writes a visible NO-FOOTPRINT row rather than silence.
W=/Users/tom/Developer/ablative/stack/aion/gate-logs/32-flatness-watch/WATCH.md
while :; do
  PID=$(cat /Users/tom/.aion/logs/aion-server.pid 2>/dev/null)
  TS=$(date -u +%Y-%m-%dT%H:%M:%SZ)
  FPLINE=$(/usr/bin/footprint "$PID" 2>/dev/null | awk '/phys_footprint:/ {print $2$3} /phys_footprint_peak:/ {print $2$3}' | paste -sd/ -)
  DU=$(du -sk /Users/tom/Developer/ablative/stack/aion/aion-data 2>/dev/null | cut -f1)
  TR=$(du -sk /Users/tom/Developer/ablative/stack/aion/gate-logs/declared-actions-fleet 2>/dev/null | cut -f1)
  echo "| $TS | ${PID:-NO-PID} | ${FPLINE:-NO-FOOTPRINT} | ${DU} KB | fleet-logs ${TR} KB |" >> "$W"
  sleep 180
done
