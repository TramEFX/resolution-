"""Extract the DaVinci Resolve Reference Manual's text, for checking what Resolve does (LOG D-017).

The manual ships with Resolve. This writes its text to target/resolve-manual/ (git-ignored: the
manual is Blackmagic's, read for reference and never copied into the repo):

    manual.txt   every page, each after a "=== page N ===" line (N is the PDF page number)
    toc.tsv      the bookmarks: level, page, title

Usage: python scripts/resolve-manual.py [path/to/DaVinci Resolve.pdf]
Needs pypdf, and fontTools for some heading fonts (`pip install pypdf fonttools`); about 2.5 minutes.
Then search it, e.g. `grep -n "Command-B" target/resolve-manual/manual.txt`.
The manual writes shortcuts the macOS way: Command is Ctrl and Option is Alt on Windows and Linux.
"""

import os
import sys

from pypdf import PdfReader

DEFAULT = r"C:\Program Files\Blackmagic Design\DaVinci Resolve\Documents\DaVinci Resolve.pdf"


def bookmarks(reader, items, level=1):
    for item in items:
        if isinstance(item, list):
            yield from bookmarks(reader, item, level + 1)
        else:
            yield level, reader.get_destination_page_number(item) + 1, item.title


def main():
    src = sys.argv[1] if len(sys.argv) > 1 else DEFAULT
    out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "target", "resolve-manual")
    os.makedirs(out, exist_ok=True)
    reader = PdfReader(src)
    title = (reader.metadata or {}).get("/Title", "?")
    with open(os.path.join(out, "toc.tsv"), "w", encoding="utf-8") as f:
        for level, page, name in bookmarks(reader, reader.outline):
            f.write(f"{level}\t{page}\t{' '.join(name.split())}\n")
    with open(os.path.join(out, "manual.txt"), "w", encoding="utf-8") as f:
        for n, page in enumerate(reader.pages, 1):
            f.write(f"\n=== page {n} ===\n{page.extract_text()}\n")
    print(f"{title}: {len(reader.pages)} pages written to {os.path.normpath(out)}")


if __name__ == "__main__":
    main()
