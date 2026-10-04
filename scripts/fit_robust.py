#!/usr/bin/env python3
"""Fit the Sony preview-matching tone curve + colour matrix used by
`raw_img_to_jpg` (see src/model/image_container/image_operations.rs).

Inputs (all in the repo root / test_out):
  * test_out/linear.bin : LibRaw decode of the RAW as *linear* sRGB (16-bit,
    u16 LE), written as [width u32][height u32][R,G,B... per pixel]. Produce it
    by opening the RAW with LibRaw, unpacking, then processing with these
    output params before `process`:
        use_camera_wb  = 1
        no_auto_bright = 1
        gamm           = (1.0, 0.0)   # identity -> linear, no gamma
        output_color   = 1            # sRGB
        output_bps     = 16
  * preview.jpg         : the largest JPEG embedded in the RAW (the camera's
    own rendering). Extracted with `jpgfromraw::process_file`.

Outputs: test_out/{T,Mr,br}.npy (tone curve, 3x3 matrix, offset) and
test_out/robust_out.jpg (the model applied to the full image).
Run `scripts/gen_image_ops.py` afterwards to regenerate image_operations.rs.
"""
import numpy as np
from PIL import Image

def load_bin(p):
    d = np.fromfile(p, dtype=np.uint16)
    w, h = int(d[0]), int(d[2])
    return d[4:].reshape(h, w, 3).astype(np.float64) / 65535.0, (w, h)

def srgb_decode(x):
    x = np.asarray(x, dtype=np.float64)
    return np.where(x <= 0.04045, x / 12.92, np.power((x + 0.055) / 1.055, 2.4))
def srgb_encode(x):
    x = np.clip(x, 0, 1)
    return np.where(x <= 0.0031308, 12.92 * x, 1.055 * np.power(x, 1 / 2.4) - 0.055)

L, (w, h) = load_bin("test_out/linear.bin")
prev = np.asarray(Image.open("preview.jpg").convert("RGB").resize((w, h), Image.LANCZOS), dtype=np.float64) / 255.0
P = srgb_decode(prev)

step = 4
Ls = L[::step, ::step].reshape(-1, 3)
Ps = P[::step, ::step].reshape(-1, 3)
print(f"samples {len(Ls)}")

W = np.array([0.2126, 0.7152, 0.0722])
Ll = Ls @ W
Pl = Ps @ W

# Step 1: shared tone curve T on luminance (linear->linear), 256 buckets
NB = 256
xi = np.clip((Ll * NB).astype(np.float64), 0, NB - 1).astype(np.int32)
sumy = np.bincount(xi, weights=Pl, minlength=NB)
cnt = np.bincount(xi, minlength=NB)
T = np.full(NB, np.nan)
good = cnt > 200
T[good] = sumy[good] / cnt[good]
idx = np.arange(NB)
T = np.interp(idx, idx[good], T[good])
for i in range(1, NB):
    if T[i] < T[i - 1]:
        T[i] = T[i - 1]
print("tone curve T: T[0]=%.3f T[64]=%.3f T[128]=%.3f T[192]=%.3f T[255]=%.3f" % (T[0], T[64], T[128], T[192], T[255]))

def apply_T(x):
    f = np.clip(x, 0, 1) * NB
    i = f.astype(np.int32)
    fr = f - i
    i2 = np.clip(i + 1, 0, NB - 1)
    i = np.clip(i, 0, NB - 1)
    return T[i] * (1 - fr) + T[i2] * fr

L2 = np.column_stack([apply_T(Ls[:, c]) for c in range(3)])
# Step 2: 3x3 + offset after tone curve
X = np.column_stack([L2, np.ones(len(L2))])
coef, *_ = np.linalg.lstsq(X, Ps, rcond=None)
M = coef[:3].T.copy()
b = coef[3].copy()
print("M =\n", M.round(4), "\nb =", b.round(4))

def pipeline(Lq):
    Lq = np.asarray(Lq, dtype=np.float64)
    Lq2 = np.column_stack([apply_T(Lq[:, c]) for c in range(3)]) if Lq.ndim == 2 else None
    return None

# Evaluate on the (subsampled) fit set
def full_pipeline(Lin):
    Lin = np.asarray(Lin, dtype=np.float64)
    L2 = np.empty_like(Lin)
    for c in range(3):
        L2[:, c] = apply_T(Lin[:, c])
    out = L2 @ M.T + b
    return srgb_encode(np.clip(out, 0, 1)) * 255

out8 = full_pipeline(Ls)
prev8 = Ps * 255
d = out8 - prev8
rmse = float(np.sqrt((d ** 2).mean())); mae = float(np.abs(d).mean())
print(f"fit-set RMSE8={rmse:.2f} MAE={mae:.2f}  per-ch bias={d.mean(0).round(2)}")

# Full-res evaluation + blurred tone RMSE
from PIL import ImageFilter
out_full = np.empty_like(L)
Lf = L.reshape(-1, 3)
of = full_pipeline(Lf).reshape(h, w, 3)
out_full = np.clip(of, 0, 255).round().astype(np.uint8)
Image.fromarray(out_full).save("test_out/robust_out.jpg", quality=95)
o = np.asarray(Image.fromarray(out_full).resize((1616, 1080), Image.LANCZOS), float)
p = np.asarray(Image.open("preview.jpg").convert("RGB"), float)
print("sharp RMSE:", round(float(np.sqrt(((o - p) ** 2).mean())), 2))
ob = np.asarray(Image.fromarray(out_full).resize((1616,1080),Image.LANCZOS).filter(ImageFilter.GaussianBlur(6)), float)
pb = np.asarray(Image.open("preview.jpg").convert("RGB").filter(ImageFilter.GaussianBlur(6)), float)
print("blurred(tone) RMSE:", round(float(np.sqrt(((ob - pb) ** 2).mean())), 2))

np.save("test_out/T.npy", T); np.save("test_out/Mr.npy", M); np.save("test_out/br.npy", b)
print("saved T.npy Mr.npy br.npy")
