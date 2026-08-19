#!/bin/zsh
# Disk-floor watch only (armed 09:2xZ). 79871 (K9 micro-fix) EXITED 09:04Z,
# envelope read, Waffles worded — his quiet-box K9 battery owns the box now.
# 25274 (#244-rem) frozen until his word. At <=8G free: standing rule = new
# conversation at Waffles' desk.
while true; do
  free=$(df -g / | awk 'NR==2 {print $4}')
  (( free <= 8 )) && { echo "DISK LOW: ${free}G free"; exit 0; }
  /bin/sleep 180
done
