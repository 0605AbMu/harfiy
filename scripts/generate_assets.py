import os
from PIL import Image, ImageDraw, ImageEnhance

SRC_PATH = r"C:\Users\a.shamsiyev\.gemini\antigravity\brain\b8227e1d-9c2a-401a-937f-49400166b9a9\.user_uploaded\media_1792646186373_5e4d4374.png"
ASSETS_DIR = r"assets"

os.makedirs(ASSETS_DIR, exist_ok=True)

# 1. Load source image
src_img = Image.open(SRC_PATH).convert("RGBA")

# 2. Crop excess white margins around the logo: (24, 24, 126, 126) -> 102x102
cropped = src_img.crop((24, 24, 126, 126))

# 3. Make outside of the rounded corners transparent
for corner in [(0, 0), (cropped.width - 1, 0), (0, cropped.height - 1), (cropped.width - 1, cropped.height - 1)]:
    ImageDraw.floodfill(cropped, corner, (0, 0, 0, 0), thresh=20)

# Save master cropped logo
cropped.save(os.path.join(ASSETS_DIR, "logo.png"))
print("Saved assets/logo.png (cropped tightly, no excess whitespace)")

# 4. Generate various sizes
sizes = [16, 24, 32, 48, 64, 128, 256]
resized_images = {}

for size in sizes:
    resized = cropped.resize((size, size), Image.Resampling.LANCZOS)
    out_file = os.path.join(ASSETS_DIR, f"logo_{size}.png")
    resized.save(out_file)
    resized_images[size] = resized
    print(f"Saved {out_file} ({size}x{size})")

# 5. Generate Windows .ico file with all standard resolutions
ico_sizes = [(16, 16), (24, 24), (32, 32), (48, 48), (64, 64), (128, 128), (256, 256)]
ico_path = os.path.join(ASSETS_DIR, "icon.ico")
cropped.save(ico_path, format="ICO", sizes=ico_sizes)
print(f"Saved {ico_path} with sizes: {ico_sizes}")

# 6. Generate Tray Active and Paused (dimmed / grayscale) raw RGBA bytes
for sz in [16, 32]:
    active_img = resized_images[sz]
    active_rgba_path = os.path.join(ASSETS_DIR, f"tray_active_{sz}.rgba")
    with open(active_rgba_path, "wb") as f:
        f.write(active_img.tobytes())
    print(f"Saved {active_rgba_path} ({len(active_img.tobytes())} bytes)")

    # Create paused / grayscale version
    r, g, b, a = active_img.split()
    gray = active_img.convert("L")
    enhancer = ImageEnhance.Brightness(gray)
    dimmed_gray = enhancer.enhance(0.70)
    paused_img = Image.merge("RGBA", (dimmed_gray, dimmed_gray, dimmed_gray, a))
    
    paused_png_path = os.path.join(ASSETS_DIR, f"logo_paused_{sz}.png")
    paused_img.save(paused_png_path)
    
    paused_rgba_path = os.path.join(ASSETS_DIR, f"tray_paused_{sz}.rgba")
    with open(paused_rgba_path, "wb") as f:
        f.write(paused_img.tobytes())
    print(f"Saved {paused_rgba_path} ({len(paused_img.tobytes())} bytes)")

print("Asset regeneration complete!")
