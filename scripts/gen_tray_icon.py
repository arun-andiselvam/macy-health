"""Generates the monochrome menu-bar template icon (water drop)."""
from PIL import Image, ImageDraw

SIZE, SS = 44, 8  # 22pt @2x, supersampled
S = SIZE * SS
img = Image.new("L", (S, S), 0)
d = ImageDraw.Draw(img)
cx, cy, r, tip = 22 * SS, 27 * SS, 12 * SS, 4 * SS
d.ellipse([cx - r, cy - r, cx + r, cy + r], fill=255)
d.polygon([(cx, tip), (cx - r * 0.93, cy - r * 0.37), (cx + r * 0.93, cy - r * 0.37)], fill=255)
# highlight cut-out so it reads as a drop, not a blob
d.arc([cx - r * 0.62, cy - r * 0.62, cx + r * 0.62, cy + r * 0.62], 100, 170, fill=0, width=int(2.4 * SS))
alpha = img.resize((SIZE, SIZE), Image.LANCZOS)
out = Image.new("RGBA", (SIZE, SIZE), (0, 0, 0, 0))
out.putalpha(alpha)
out.save("src-tauri/icons/tray.png")
