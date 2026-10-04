#!/usr/bin/env python3
"""Compare a processed output image against the camera preview JPEG."""
import sys
import numpy as np
from PIL import Image

def load(path, size=None):
    img = Image.open(path).convert("RGB")
    if size:
        img = img.resize(size, Image.LANCZOS)
    return np.asarray(img, dtype=np.float64)

def main(out_path, ref_path):
    ref_img = Image.open(ref_path).convert("RGB")
    size = ref_img.size
    out = load(out_path, size)
    ref = np.asarray(ref_img, dtype=np.float64)

    diff = out - ref
    rmse = np.sqrt(np.mean(diff ** 2))
    mae = np.mean(np.abs(diff))
    psnr = 99.0 if rmse == 0 else 20 * np.log10(255.0 / rmse)

    # Per-channel stats
    chan_rmse = np.sqrt(np.mean(diff ** 2, axis=(0, 1)))
    chan_mean_bias = diff.mean(axis=(0, 1))

    print(f"comparing: {out_path} vs {ref_path} (resized to {size[0]}x{size[1]})")
    print(f"RMSE: {rmse:.2f}   MAE: {mae:.2f}   PSNR: {psnr:.2f} dB")
    print(f"per-channel RMSE (R,G,B): {chan_rmse.round(2)}")
    print(f"per-channel bias  (R,G,B): {chan_mean_bias.round(2)}")

    # Luminance histogram difference
    out_l = out.mean(axis=2)
    ref_l = ref.mean(axis=2)
    ho, _ = np.histogram(out_l, bins=32, range=(0, 256))
    hr, _ = np.histogram(ref_l, bins=32, range=(0, 256))
    ho = ho / ho.sum(); hr = hr / hr.sum()
    print(f"histogram L1 diff (32 bins): {np.abs(ho - hr).sum():.4f}")

    # Save a side-by-side and a diff visualization
    vis = np.clip(np.abs(diff) * 3.0, 0, 255).astype(np.uint8)
    side = np.concatenate([out.astype(np.uint8), ref.astype(np.uint8), vis], axis=1)
    Image.fromarray(side).save("test_out/compare_vis.jpg", quality=90)
    print("saved test_out/compare_vis.jpg [output | preview | |diff|x3]")

if __name__ == "__main__":
    out = sys.argv[1] if len(sys.argv) > 1 else "test_out/current_full.jpg"
    ref = sys.argv[2] if len(sys.argv) > 2 else "preview.jpg"
    main(out, ref)
