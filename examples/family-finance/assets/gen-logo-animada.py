#!/usr/bin/env python3
"""Writes the animated logo of Raven Finanças as a Lottie file.

The motion is worked out here, from the same curves as assets/logo.svg, and sampled
into keyframes: python3 gen-logo-animada.py > logo-animada.json
"""
import json
import math
import sys

FPS = 60
END_MS = 1900
PAD = 120  # Room for what passes outside the tile.
SIDE = 1024 + PAD * 2
STEP = 2  # Frames between samples.


def clamp(x):
    return min(1.0, max(0.0, x))


def span(t, a, b):
    return clamp((t - a) / (b - a))


def lerp(a, b, k):
    return a + (b - a) * k


def out_expo(k):
    return 1.0 if k >= 1 else 1 - 2 ** (-10 * k)


def out_back(k):
    c = 1.35
    return 1 + (c + 1) * (k - 1) ** 3 + c * (k - 1) ** 2


def in_out(k):
    return 4 * k**3 if k < 0.5 else 1 - (-2 * k + 2) ** 3 / 2


FINAL = [(196,884),(262,772),(250,628),(276,500),(304,356),(424,226),(566,232),(640,236),(700,274),(738,330),(798,356),(858,382),(918,408),(855,429),(791,449),(728,470),(698,488),(682,516),(672,566),(646,704),(556,816),(416,852),(344,870),(268,880),(196,884)]
SPINE = [(196,884),(250,600),(470,250),(918,408)]
WING = {"c": True, "v": [[322,768],[432,490]], "i": [[0,0],[-66,76]], "o": [[34,-108],[36,76]]}
WING_V = {"c": True, "v": [[322,768],[432,490]], "i": [[144,-68],[-66,76]], "o": [[34,-108],[36,76]]}


def spine(s):
    u = 1 - s
    return tuple(u*u*u*SPINE[0][a] + 3*u*u*s*SPINE[1][a] + 3*u*s*s*SPINE[2][a] + s*s*s*SPINE[3][a] for a in (0, 1))


def body_at(t):
    pts = []
    n = len(FINAL) - 1
    for i, p in enumerate(FINAL):
        u = i / n
        s = u * 2 if u <= 0.5 else (1 - u) * 2
        f = spine(s)
        k = out_back(clamp((t - (470 + s * 170)) / 430))
        pts.append((lerp(f[0], p[0], k), lerp(f[1], p[1], k)))
    return pts


def bezier_of(pts):
    # Vertices with handles, from the run of cubic points.
    v = [pts[i] for i in range(0, len(pts) - 1, 3)]
    count = len(v)
    o, i_ = [], []
    for k in range(count):
        at = k * 3
        c1 = pts[at + 1]
        o.append((c1[0] - v[k][0], c1[1] - v[k][1]))
        prev_c2 = pts[at - 1] if k > 0 else pts[len(pts) - 2]
        i_.append((prev_c2[0] - v[k][0], prev_c2[1] - v[k][1]))
    r = lambda q: [round(q[0], 2), round(q[1], 2)]
    return {"c": True, "v": [r(q) for q in v], "i": [r(q) for q in i_], "o": [r(q) for q in o]}


def frames():
    f = 0
    last = END_MS * FPS / 1000
    while f < last:
        yield f, f * 1000 / FPS
        f += STEP
    yield last, END_MS


LINEAR = {"o": {"x": [0.0], "y": [0.0]}, "i": {"x": [1.0], "y": [1.0]}}


def keyed(fn):
    """A property sampled over time, with repeats left out."""
    keys = []
    last = None
    pending = None
    for frame, t in frames():
        value = fn(t)
        value = value if isinstance(value, list) else [value]
        value = [round(v, 3) for v in value]
        if value == last:
            pending = (frame, value)
            continue
        if pending is not None:
            keys.append({"t": round(pending[0], 2), "s": pending[1], **LINEAR})
            pending = None
        keys.append({"t": round(frame, 2), "s": value, **LINEAR})
        last = value
    if pending is not None:
        keys.append({"t": round(pending[0], 2), "s": pending[1], **LINEAR})
    if len(keys) == 1:
        return {"a": 0, "k": keys[0]["s"] if len(keys[0]["s"]) > 1 else keys[0]["s"][0]}
    return {"a": 1, "k": keys}


def keyed_shape(fn):
    keys = []
    for frame, t in frames():
        keys.append({"t": round(frame, 2), "s": [fn(t)], **LINEAR})
    return {"a": 1, "k": keys}


def fixed(v):
    return {"a": 0, "k": v}


def transform(anchor=(0, 0), position=None, scale=None, rotation=None, opacity=None):
    return {
        "a": fixed(list(anchor)),
        "p": position or fixed([anchor[0], anchor[1]]),
        "s": scale or fixed([100, 100]),
        "r": rotation or fixed(0),
        "o": opacity or fixed(100),
    }


def group(*items):
    tr = {"ty": "tr", "a": fixed([0, 0]), "p": fixed([0, 0]), "s": fixed([100, 100]), "r": fixed(0), "o": fixed(100)}
    return {"ty": "gr", "it": list(items) + [tr]}


def fill(hex_color):
    c = [int(hex_color[i:i+2], 16) / 255 for i in (1, 3, 5)]
    return {"ty": "fl", "c": fixed([round(x, 4) for x in c] + [1]), "o": fixed(100), "r": 1}


def stroke(hex_color, width, opacity=None):
    c = [int(hex_color[i:i+2], 16) / 255 for i in (1, 3, 5)]
    return {"ty": "st", "c": fixed([round(x, 4) for x in c] + [1]), "o": opacity or fixed(100), "w": width if isinstance(width, dict) else fixed(width), "lc": 2, "lj": 2, "ml": 4}


def gradient(kind_key, start, end, stops):
    flat = []
    for off, hex_color in stops:
        flat += [off] + [round(int(hex_color[i:i+2], 16) / 255, 4) for i in (1, 3, 5)]
    return {"ty": kind_key, "t": 1, "s": fixed(list(start)), "e": fixed(list(end)), "o": fixed(100), "r": 1, "g": {"p": len(stops), "k": fixed(flat)}}


def layer(index, name, shapes, ks, **extra):
    return {"ty": 4, "ind": index, "nm": name, "ks": ks, "ip": 0, "op": END_MS * FPS / 1000 + 1, "st": 0, "shapes": shapes, **extra}


SHIFT = fixed([PAD, PAD])
body_final = bezier_of(FINAL)
layers = []


def plain(anchor=(0, 0), **kw):
    kw.setdefault("position", fixed([anchor[0] + PAD, anchor[1] + PAD]))
    return transform(anchor, **kw)


n = 1
# The eye, on top of everything.
eye = lambda t: 2 * 28 * out_back(span(t, 1340, 1560)) if t >= 1340 else 0
layers.append(layer(n, "olho", [group({"ty": "el", "p": fixed([584, 340]), "s": keyed(lambda t: [eye(t), eye(t)])}, fill("#ffd76a"))], plain())); n += 1

# Sparks.
burst = lambda t: out_expo(span(t, 1040, 1420))
for i, angle in enumerate((-2.5, -1.7, -0.9, -0.2, 0.55, 1.3)):
    color = ("#ffd76a", "#f2a93b", "#ffd76a", "#ffffff", "#f2a93b", "#ffd76a")[i]
    reach = lambda t, i=i: 130 + burst(t) * (150 + (i % 3) * 45)
    pos = keyed(lambda t, angle=angle, reach=reach: [806 + math.cos(angle) * reach(t), 520 + math.sin(angle) * reach(t)])
    size = lambda t: 26 * (1 - span(t, 1200, 1480))
    layers.append(layer(n, f"faisca {i}", [group({"ty": "el", "p": pos, "s": keyed(lambda t: [size(t), size(t)])}, fill(color))], plain(opacity=keyed(lambda t: (100 * (1 - span(t, 1240, 1480))) if t >= 1040 else 0)))); n += 1

# The wing, with its feathers of light.
swing = lambda t: -30 * math.exp(-5.5 * (t - 900) / 1000) * math.cos(11 * (t - 900) / 1000) if t >= 900 else 0
opening = lambda t: span(t, 880, 1000)
wing_path = {"ty": "sh", "ks": fixed(WING_V)}
layers.append(layer(n, "asa", [group(wing_path, fill("#2a3040"))], plain((432, 490), rotation=keyed(swing), scale=keyed(lambda t: [100 * lerp(0.2, 1, opening(t)), 100 * lerp(0.4, 1, opening(t))]), opacity=keyed(lambda t: 100 if t >= 880 else 0)))); n += 1
feather = lambda t: span(t, 1000, 1380)
layers.append(layer(n, "pena 1", [group(wing_path, fill("#7f93f5"))], plain((432, 490), rotation=keyed(lambda t: swing(t) * 1.5 - 14 * (1 - feather(t))), opacity=keyed(lambda t: 70 * (1 - feather(t)) if t >= 900 else 0)))); n += 1
layers.append(layer(n, "pena 2", [group(wing_path, fill("#4c66ee"))], plain((432, 490), rotation=keyed(lambda t: swing(t) * 2.1 - 28 * (1 - feather(t))), opacity=keyed(lambda t: 45 * (1 - feather(t)) if t >= 900 else 0)))); n += 1

# The sheen, seen only through the body.
sheen = lambda t: in_out(span(t, 1230, 1720))
matte_shapes = [group({"ty": "sh", "ks": fixed(body_final)}, fill("#000000"))]
layers.append(layer(n, "corpo (recorte)", matte_shapes, plain(), td=1)); n += 1
band = {"ty": "gf", "t": 1, "s": keyed(lambda t: [lerp(-300, 900, sheen(t)), 200]), "e": keyed(lambda t: [lerp(100, 1300, sheen(t)), 900]), "o": fixed(100), "r": 1,
        "g": {"p": 3, "k": fixed([0.38, 0.298, 0.4, 0.933, 0.5, 0.498, 0.576, 0.961, 0.62, 0.298, 0.4, 0.933, 0.38, 0, 0.5, 0.75, 0.62, 0])}}
rect = {"ty": "rc", "p": fixed([550, 550]), "s": fixed([900, 800]), "r": fixed(0)}
layers.append(layer(n, "brilho", [group(rect, band)], plain(opacity=keyed(lambda t: 100 if 1230 <= t < 1720 else 0)), tt=1)); n += 1

# The body, opening out from the gesture.
layers.append(layer(n, "corpo", [group({"ty": "sh", "ks": keyed_shape(lambda t: bezier_of(body_at(t)))}, fill("#12151c"))], plain(opacity=keyed(lambda t: 100 if t >= 470 else 0)))); n += 1

# The gesture of light: a wide glow, its line and a golden head.
SWOOSH = {"c": False, "v": [[1120,60],[130,300],[196,884],[918,408]], "i": [[0,0],[120,-430],[-86,-114],[-448,-158]], "o": [[-300,-210],[-50,220],[54,-284],[0,0]]}
head = lambda t: in_out(span(t, 0, 640))
tail = lambda t: in_out(span(t, 150, 790))
fade = lambda t: 100 * (1 - span(t, 700, 800))


def trail(name, color, width, opacity, start):
    trim = {"ty": "tm", "s": keyed(lambda t: 100 * start(t)), "e": keyed(lambda t: 100 * head(t)), "o": fixed(0), "m": 1}
    return layer(n, name, [group({"ty": "sh", "ks": fixed(SWOOSH)}, trim, stroke(color, width))], plain(opacity=keyed(lambda t: fade(t) * opacity if t < 800 else 0)))


layers.append(trail("cabeca do gesto", "#ffd76a", 12, 1.0, lambda t: max(tail(t), head(t) - 0.05))); n += 1
layers.append(trail("gesto", "#4c66ee", 20, 1.0, tail)); n += 1
layers.append(trail("brilho do gesto", "#9db0ff", 44, 0.4, tail)); n += 1

# The coin falls, turning, and lands on the beak.
fall = lambda t: span(t, 760, 1040)
coin_x = lambda t: lerp(250, 0, fall(t))
coin_y = lambda t: -620 * (1 - fall(t)) ** 2
turn = lambda t: 1 if t >= 1040 else abs(math.cos(fall(t) * math.pi * 3))
squash = lambda t: math.exp(-(t - 1040) / 90) * math.cos((t - 1040) / 45) * 0.16 if t >= 1040 else 0
coin_shapes = [
    group({"ty": "el", "p": fixed([806, 520]), "s": fixed([136, 136])}, stroke("#b9791c", 16)),
    group({"ty": "el", "p": fixed([806, 520]), "s": fixed([200, 200])}, gradient("gf", (806, 420), (806, 620), [(0, "#ffd76a"), (1, "#f2a93b")])),
]
layers.append(layer(n, "moeda", coin_shapes, transform((806, 520), position=keyed(lambda t: [806 + PAD + coin_x(t), 520 + PAD + coin_y(t)]), scale=keyed(lambda t: [100 * max(0.06, turn(t)) * (1 + squash(t)), 100 * (1 - squash(t))]), opacity=keyed(lambda t: 100 if t >= 760 else 0)))); n += 1

# The ripple of its touch.
ripple = lambda t: span(t, 1040, 1480)
ring = lambda t: 2 * (100 + ripple(t) * 210)
layers.append(layer(n, "onda", [group({"ty": "el", "p": fixed([806, 520]), "s": keyed(lambda t: [ring(t), ring(t)])}, stroke("#ffd76a", keyed(lambda t: 10 * (1 - ripple(t)) + 1)))], plain(opacity=keyed(lambda t: 80 * (1 - ripple(t)) if t >= 1040 else 0)))); n += 1

# The tile blooms from where the coin touched, seen through a growing circle.
K = 0.5522848


def circle_at(t):
    r = max(0.01, out_expo(span(t, 1040, 1560)) * 1250)
    x, y, h = 760, 500, r * K
    return {"c": True, "v": [[x, y-r], [x+r, y], [x, y+r], [x-r, y]], "i": [[-h, 0], [0, -h], [h, 0], [0, h]], "o": [[h, 0], [0, h], [-h, 0], [0, -h]]}


tile_scale = lambda t: 100 * lerp(0.9, 1, out_back(span(t, 1040, 1500)))
tile = group({"ty": "rc", "p": fixed([512, 512]), "s": fixed([1024, 1024]), "r": fixed(228)}, gradient("gf", (0, 0), (1024, 1024), [(0, "#5f78f5"), (1, "#3348bf")]))
layers.append(layer(n, "bloom (recorte)", [group({"ty": "sh", "ks": keyed_shape(circle_at)}, fill("#000000"))], plain(), td=1)); n += 1
layers.append(layer(n, "base", [tile], plain((512, 512), scale=keyed(lambda t: [tile_scale(t), tile_scale(t)])), tt=1)); n += 1

document = {"v": "5.7.0", "nm": "Raven Finanças", "fr": FPS, "ip": 0, "op": END_MS * FPS / 1000, "w": SIDE, "h": SIDE, "ddd": 0, "assets": [], "layers": layers}
json.dump(document, sys.stdout, ensure_ascii=False, separators=(",", ":"))
