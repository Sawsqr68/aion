#!/bin/zsh
# Wake when free disk falls to 18G (decision headroom before the 15G
# pause-a-builder lever) or a builder exits first (then this is moot).
while true; do
  free=$(df -g / | awk 'NR==2 {print $4}')
  if (( free <= 18 )); then
    echo "DISK LOW: ${free}G free"
    exit 0
  fi
  if ! kill -0 69624 2>/dev/null || ! kill -0 76992 2>/dev/null; then
    echo "builder exited with ${free}G free — disk watch moot"
    exit 0
  fi
  /bin/sleep 180
done
