"""Generates the 1024px app icon (macOS squircle, drop + reminder ring).
Feed the result to `pnpm tauri icon app-icon.png` to produce every size."""
import math
from PIL import Image, ImageDraw, ImageFilter

S = 1024
SS = 4  # supersample
W = S * SS

def squircle(size, n=4.2, steps=720):
    r = size / 2
    pts = []
    for i in range(steps):
        t = 2 * math.pi * i / steps
        c, s = math.cos(t), math.sin(t)
        x = r * math.copysign(abs(c) ** (2 / n), c)
        y = r * math.copysign(abs(s) ** (2 / n), s)
        pts.append((x, y))
    return pts

# macOS grid: 824px body centred on a 1024 canvas.
body = 824 * SS
off = (W - body) / 2
shape = [(x + W / 2, y + W / 2) for x, y in squircle(body)]

# Vertical gradient: sky blue → teal.
top, bottom = (40, 142, 255), (38, 196, 180)
grad = Image.new("RGB", (1, W))
for y in range(W):
    t = min(1, max(0, (y - off) / body))
    grad.putpixel((0, y), tuple(int(a + (b - a) * t) for a, b in zip(top, bottom)))
grad = grad.resize((W, W))

mask = Image.new("L", (W, W), 0)
ImageDraw.Draw(mask).polygon(shape, fill=255)

art = Image.new("RGBA", (W, W), (0, 0, 0, 0))
d = ImageDraw.Draw(art)
cx, cy = W / 2, W / 2 + 10 * SS

# Reminder ring: a 3/4 arc with a rounded end, like the eye-rest countdown.
ring_r, ring_w = 300 * SS, 44 * SS
box = [cx - ring_r, cy - ring_r, cx + ring_r, cy + ring_r]
d.arc(box, start=-90, end=180, fill=(255, 255, 255, 235), width=ring_w)
for ang in (-90, 180):
    a = math.radians(ang)
    ex, ey = cx + (ring_r - ring_w / 2) * math.cos(a), cy + (ring_r - ring_w / 2) * math.sin(a)
    d.ellipse([ex - ring_w / 2, ey - ring_w / 2, ex + ring_w / 2, ey + ring_w / 2], fill=(255, 255, 255, 235))

# Water drop: circle plus the two tangent lines from the tip, so the
# outline has no corners where they meet.
dr = 136 * SS
dcy = cy + 60 * SS
tip = dcy - 292 * SS
dist = dcy - tip
phi = math.acos(dr / dist)
d.ellipse([cx - dr, dcy - dr, cx + dr, dcy + dr], fill=(255, 255, 255, 255))
d.polygon(
    [(cx, tip), (cx - dr * math.sin(phi), dcy - dr * math.cos(phi)), (cx + dr * math.sin(phi), dcy - dr * math.cos(phi))],
    fill=(255, 255, 255, 255),
)
# Highlight inside the drop, in the gradient colour.
hl = 84 * SS
d.arc([cx - hl, dcy - hl, cx + hl, dcy + hl], start=100, end=165, fill=(52, 170, 225, 255), width=20 * SS)

icon = Image.new("RGBA", (W, W), (0, 0, 0, 0))
icon.paste(grad, (0, 0), mask)
icon = Image.alpha_composite(icon, Image.composite(art, Image.new("RGBA", (W, W)), mask))

# Soft drop shadow under the body.
shadow = Image.new("RGBA", (W, W), (0, 0, 0, 0))
sm = mask.filter(ImageFilter.GaussianBlur(14 * SS)).point(lambda v: int(v * 0.28))
shadow.putalpha(sm)
shadow = shadow.transform(shadow.size, Image.AFFINE, (1, 0, 0, 0, 1, -10 * SS))
out = Image.alpha_composite(shadow, icon).resize((S, S), Image.LANCZOS)
out.save("app-icon.png")
print("wrote app-icon.png")
