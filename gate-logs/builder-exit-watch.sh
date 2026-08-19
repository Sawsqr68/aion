#!/bin/zsh
# Wake when either flying builder exits: #241-remediation (69624) or
# K9-bootstrap-standing (76992). Prints which one(s).
while true; do
  a=$(kill -0 69624 2>/dev/null && echo alive || echo gone)
  b=$(kill -0 76992 2>/dev/null && echo alive || echo gone)
  if [[ "$a" == gone || "$b" == gone ]]; then
    echo "241-remediation(69624)=$a k9-remediation(76992)=$b"
    exit 0
  fi
  /bin/sleep 120
done
