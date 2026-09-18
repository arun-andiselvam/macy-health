"""Generates the app's UI sounds (soft bell tones), so no third-party audio is shipped."""
import math, struct, wave

RATE = 22050

def bell(freq, start, t):
    if t < start:
        return 0.0
    t -= start
    attack = min(1.0, t / 0.006)
    decay = math.exp(-t / 0.38)
    partials = (1.0, 1.0), (2.0, 0.28), (3.01, 0.08), (4.2, 0.03)
    return attack * decay * sum(a * math.sin(2 * math.pi * freq * m * t) for m, a in partials)

def render(path, notes, seconds, gain=0.32):
    frames = bytearray()
    for i in range(int(RATE * seconds)):
        t = i / RATE
        s = sum(bell(f, st, t) for f, st in notes) * gain
        fade = min(1.0, (seconds - t) / 0.15)  # avoid a click at the end
        frames += struct.pack("<h", int(max(-1, min(1, s * fade)) * 32767))
    with wave.open(path, "wb") as w:
        w.setnchannels(1); w.setsampwidth(2); w.setframerate(RATE)
        w.writeframes(bytes(frames))

# Popup appears: two gentle notes, G5 → C6.
render("src-tauri/sounds/chime.wav", [(783.99, 0.0), (1046.5, 0.14)], 1.3)
# Eye-rest countdown finished: rising C6 → E6 → G6.
render("src-tauri/sounds/complete.wav", [(1046.5, 0.0), (1318.5, 0.1), (1568.0, 0.2)], 1.4, gain=0.26)
