"""Generate the MSIX visual assets in packaging/windows/msix/Assets from the brand logos.

Run from the repository root: python scripts/generate_msix_assets.py
The scale- and targetsize- qualified files are indexed into resources.pri by
packaging/windows/build_msix.ps1 (makepri), so the manifest names them without qualifiers.
"""

import os

from PIL import Image

GLYPH_PATH = os.path.join("assets", "store", "harfiy_store_transparent_2160x2160.png")
APP_ICON_PATH = os.path.join("assets", "app_icon_1024.png")
OUT_DIR = os.path.join("packaging", "windows", "msix", "Assets")


def load_glyph():
    """Load the transparent logo glyph, cropped to its visible pixels."""
    img = Image.open(GLYPH_PATH).convert("RGBA")
    return img.crop(img.getbbox())


def place(img, width, height, fill):
    """Center `img` on a transparent canvas, scaled to `fill` of the canvas height."""
    canvas = Image.new("RGBA", (width, height), (0, 0, 0, 0))
    target_h = max(1, round(height * fill))
    target_w = max(1, round(img.width * target_h / img.height))
    if target_w > width * fill:
        target_w = max(1, round(width * fill))
        target_h = max(1, round(img.height * target_w / img.width))
    scaled = img.resize((target_w, target_h), Image.LANCZOS)
    canvas.alpha_composite(scaled, ((width - target_w) // 2, (height - target_h) // 2))
    return canvas


def save(img, name):
    img.save(os.path.join(OUT_DIR, name), optimize=True)
    print("Saved", name, img.size)


def main():
    os.makedirs(OUT_DIR, exist_ok=True)
    glyph = load_glyph()
    app_icon = Image.open(APP_ICON_PATH).convert("RGBA")

    # App list, taskbar and title bar icon
    for scale in (100, 200, 400):
        size = 44 * scale // 100
        save(place(glyph, size, size, 0.8), f"Square44x44Logo.scale-{scale}.png")
    for size in (16, 24, 32, 48, 256):
        icon = place(glyph, size, size, 0.9)
        save(icon, f"Square44x44Logo.targetsize-{size}.png")
        save(icon, f"Square44x44Logo.targetsize-{size}_altform-unplated.png")

    # Start tiles
    for scale in (100, 200, 400):
        side = 150 * scale // 100
        save(place(glyph, side, side, 0.5), f"Square150x150Logo.scale-{scale}.png")
        save(
            place(glyph, 310 * scale // 100, 150 * scale // 100, 0.5),
            f"Wide310x150Logo.scale-{scale}.png",
        )

    # Package logo shown by the installer and the Store
    for scale in (100, 200, 400):
        size = 50 * scale // 100
        save(app_icon.resize((size, size), Image.LANCZOS), f"StoreLogo.scale-{scale}.png")


if __name__ == "__main__":
    main()
