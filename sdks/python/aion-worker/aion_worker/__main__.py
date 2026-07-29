"""`python -m aion_worker <script.py>` — serve a decorated script as a worker.

The same entry point is installed as the `aion-worker` console script, so a
`pip install aion-worker` gives both spellings and neither is the odd one out.
"""

from __future__ import annotations

import sys

from .host import main

if __name__ == "__main__":
    sys.exit(main())
