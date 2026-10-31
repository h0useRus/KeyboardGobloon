"""Builds assets/icon/KeyboardGobloon.ico and KeyboardGobloon-256.png from KeyboardGobloon.svg.

Renders the SVG with headless Chrome (no SVG library needed) and packs every size with Pillow.
Run from the repository root: python tools/icon.py
"""

import pathlib
import subprocess
import tempfile

from PIL import Image

ICON = pathlib.Path("assets/icon")
CHROME = r"C:\Program Files\Google\Chrome\Application\chrome.exe"
SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]
RENDER = 1024


def render(svg: pathlib.Path, png: pathlib.Path) -> None:
    page = png.with_suffix(".html")
    page.write_text(
        '<html><body style="margin:0;background:transparent">'
        f'<img src="{svg.resolve().as_uri()}" width="{RENDER}" height="{RENDER}"></body></html>',
        encoding="utf-8",
    )
    subprocess.run(
        [
            CHROME,
            "--headless=new",
            "--disable-gpu",
            "--hide-scrollbars",
            "--default-background-color=00000000",
            f"--window-size={RENDER},{RENDER}",
            f"--screenshot={png}",
            page.as_uri(),
        ],
        check=True,
        capture_output=True,
    )


def main() -> None:
    with tempfile.TemporaryDirectory() as tmp:
        png = pathlib.Path(tmp) / "icon.png"
        render(ICON / "KeyboardGobloon.svg", png)
        big = Image.open(png).convert("RGBA")
    images = [big.resize((n, n), Image.LANCZOS) for n in SIZES]
    images[-1].save(ICON / "KeyboardGobloon-256.png")
    images[-1].save(
        ICON / "KeyboardGobloon.ico",
        sizes=[(n, n) for n in SIZES],
        append_images=images[:-1],
    )
    print("sizes:", sorted(Image.open(ICON / "KeyboardGobloon.ico").info["sizes"]))


if __name__ == "__main__":
    main()
