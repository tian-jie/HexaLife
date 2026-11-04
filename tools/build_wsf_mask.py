#!/usr/bin/env python3
"""Build HLHB habitability mask from WSF2019 (GeoTIFF tiles or thumbnails)."""

from __future__ import annotations

import argparse
import http.client
import re
import sys
import threading
import time
import urllib.error
import urllib.request
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path

import numpy as np
from PIL import Image

try:
    import rasterio
    from rasterio.transform import xy as raster_xy
except ImportError:  # pragma: no cover - optional until --source tif
    rasterio = None  # type: ignore[assignment,misc]
    raster_xy = None  # type: ignore[assignment,misc]

MAGIC = b"HLHB"
BASE_URL = "https://download.geoservice.dlr.de/WSF2019/files/"
THUMB_RE = re.compile(r"WSF2019_v1_(-?\d+)_(-?\d+)_thumbnail\.png")
TIF_RE = re.compile(r"WSF2019_v1_(-?\d+)_(-?\d+)\.tif")
THUMB_PAD_DEG = 0.1


def fetch_directory_names(pattern: re.Pattern[str]) -> list[tuple[int, int, str]]:
    with urllib.request.urlopen(BASE_URL, timeout=120) as resp:
        html = resp.read().decode("utf-8", errors="replace")
    out: list[tuple[int, int, str]] = []
    seen: set[str] = set()
    for m in pattern.finditer(html):
        name = m.group(0)
        if name in seen:
            continue
        seen.add(name)
        lon = int(m.group(1))
        lat = int(m.group(2))
        out.append((lon, lat, name))
    return out


def download_one(name: str, dest: Path) -> None:
    dest.parent.mkdir(parents=True, exist_ok=True)
    if dest.is_file() and dest.stat().st_size > 0:
        if name.endswith(".tif"):
            if rasterio is not None:
                try:
                    with rasterio.open(dest) as ds:
                        ds.read(1, window=((0, 1), (0, 1)))
                    return
                except Exception:
                    pass
        else:
            try:
                with Image.open(dest) as im:
                    im.verify()
                return
            except Exception:
                pass
    url = BASE_URL + name
    last_err: Exception | None = None
    for attempt in range(8):
        try:
            with urllib.request.urlopen(url, timeout=300) as resp:
                expected = resp.headers.get("Content-Length")
                data = resp.read()
            if expected is not None and len(data) != int(expected):
                raise OSError(
                    f"short read for {name}: got {len(data)} bytes, expected {expected}"
                )
            dest.write_bytes(data)
            break
        except (
            urllib.error.HTTPError,
            urllib.error.URLError,
            TimeoutError,
            http.client.IncompleteRead,
            OSError,
        ) as e:
            last_err = e
            if dest.is_file():
                dest.unlink(missing_ok=True)
            time.sleep(min(2 ** attempt, 30))
    else:
        raise last_err  # type: ignore[misc]
    if name.endswith(".tif"):
        if rasterio is None:
            raise RuntimeError("rasterio is required for GeoTIFF tiles")
        with rasterio.open(dest) as ds:
            ds.read(1, window=((0, 1), (0, 1)))
    else:
        with Image.open(dest) as im:
            im.load()


def lon_lat_to_cell(
    lon: float, lat: float, width: int, height: int
) -> tuple[int, int] | None:
    if lon < -180.0 or lon >= 180.0 or lat < -90.0 or lat > 90.0:
        return None
    q = int((lon + 180.0) * width / 360.0)
    r = int((90.0 - lat) * height / 180.0)
    if q < 0 or q >= width or r < 0 or r >= height:
        return None
    return q, r


def set_bit(bits: bytearray, index: int, lock: threading.Lock) -> None:
    byte_i = index // 8
    bit_i = index % 8
    mask = 1 << bit_i
    with lock:
        bits[byte_i] |= mask


def set_bits_unique(bits: bytearray, indices: np.ndarray, lock: threading.Lock) -> None:
    if indices.size == 0:
        return
    uniq = np.unique(indices)
    with lock:
        for idx in uniq:
            idx = int(idx)
            bits[idx // 8] |= 1 << (idx % 8)


def cells_from_lon_lat(
    lon: np.ndarray, lat: np.ndarray, width: int, height: int
) -> np.ndarray:
    q = np.floor((lon + 180.0) * width / 360.0).astype(np.int64)
    r = np.floor((90.0 - lat) * height / 180.0).astype(np.int64)
    ok = (lon >= -180.0) & (lon < 180.0) & (lat >= -90.0) & (lat <= 90.0)
    ok &= (q >= 0) & (q < width) & (r >= 0) & (r < height)
    idx = r * width + q
    return idx[ok]


def apply_geotiff(
    bits: bytearray,
    width: int,
    height: int,
    path: Path,
    lock: threading.Lock,
) -> None:
    if rasterio is None or raster_xy is None:
        raise RuntimeError("rasterio is required for --source tif")
    with rasterio.open(path) as ds:
        data = ds.read(1)
        rows, cols = np.nonzero(data == 255)
        if rows.size == 0:
            return
        lons, lats = raster_xy(ds.transform, rows, cols, offset="center")
        lons = np.asarray(lons, dtype=np.float64)
        lats = np.asarray(lats, dtype=np.float64)
    indices = cells_from_lon_lat(lons, lats, width, height)
    set_bits_unique(bits, indices, lock)


def apply_thumbnail(
    bits: bytearray,
    width: int,
    height: int,
    lon: int,
    lat: int,
    path: Path,
    lock: threading.Lock,
) -> None:
    with Image.open(path) as im:
        im = im.convert("RGB")
        w_px, h_px = im.size
        raw = im.tobytes()
    lon_min = lon + THUMB_PAD_DEG
    lon_max = lon + 2.0 - THUMB_PAD_DEG
    lat_max = lat + 2.0 - THUMB_PAD_DEG
    lat_min = lat + THUMB_PAD_DEG
    lon_span = lon_max - lon_min
    lat_span = lat_max - lat_min
    row_stride = w_px * 3
    pending: list[int] = []
    for py in range(h_px):
        lat_c = lat_max - (py + 0.5) * lat_span / h_px
        row = py * row_stride
        for px in range(w_px):
            off = row + px * 3
            if raw[off] != 255 or raw[off + 1] != 255 or raw[off + 2] != 255:
                continue
            lon_c = lon_min + (px + 0.5) * lon_span / w_px
            cell = lon_lat_to_cell(lon_c, lat_c, width, height)
            if cell is None:
                continue
            q, r = cell
            pending.append(r * width + q)
    if pending:
        set_bits_unique(bits, np.asarray(pending, dtype=np.int64), lock)


def write_mask(path: Path, width: int, height: int, bits: bytearray) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    with path.open("wb") as f:
        f.write(MAGIC)
        f.write(width.to_bytes(4, "little"))
        f.write(height.to_bytes(4, "little"))
        f.write(bits)


_POPCOUNT = tuple(bin(i).count("1") for i in range(256))


def habitable_count(bits: bytearray, total: int) -> int:
    full_bytes = total // 8
    n = sum(_POPCOUNT[b] for b in bits[:full_bytes])
    rem = total % 8
    if rem:
        n += _POPCOUNT[bits[full_bytes] & ((1 << rem) - 1)]
    return n


def build_mask(
    width: int,
    height: int,
    out: Path,
    source: str,
    cache_dir: Path,
    workers: int,
) -> int:
    total = width * height
    n_bytes = (total + 7) // 8
    bits = bytearray(n_bytes)
    lock = threading.Lock()

    if source == "tif":
        tiles = fetch_directory_names(TIF_RE)
        apply = lambda _lon, _lat, p: apply_geotiff(bits, width, height, p, lock)
    elif source == "thumbnail":
        tiles = fetch_directory_names(THUMB_RE)
        apply = lambda lon, lat, p: apply_thumbnail(
            bits, width, height, lon, lat, p, lock
        )
    else:
        print(f"unknown source: {source}", file=sys.stderr)
        return 1

    if not tiles:
        print("No tiles found on directory page", file=sys.stderr)
        return 1

    tiles = sorted(tiles, key=lambda t: (t[1], t[0]))
    print(f"source={source} tiles={len(tiles)} grid={width}x{height}", flush=True)

    def job(item: tuple[int, int, str]) -> str:
        lon, lat, name = item
        dest = cache_dir / name
        download_one(name, dest)
        apply(lon, lat, dest)
        return name

    done = 0
    with ThreadPoolExecutor(max_workers=workers) as pool:
        futures = [pool.submit(job, t) for t in tiles]
        for fut in as_completed(futures):
            fut.result()
            done += 1
            if done % 50 == 0 or done == len(tiles):
                print(f"  tiles {done}/{len(tiles)}", flush=True)

    count = habitable_count(bits, total)
    print(f"habitable cells: {count} / {total} ({100.0 * count / total:.4f}%)", flush=True)
    write_mask(out, width, height, bits)
    print(f"wrote {out} ({out.stat().st_size} bytes)", flush=True)

    if count == 0 or count == total:
        print("habitable count must be strictly between 0 and total", file=sys.stderr)
        return 1
    return 0


def main() -> int:
    parser = argparse.ArgumentParser(description="Build WSF2019 habitability mask")
    parser.add_argument("--width", type=int)
    parser.add_argument("--height", type=int)
    parser.add_argument("--out", type=Path)
    parser.add_argument(
        "--source",
        choices=("tif", "thumbnail"),
        default="tif",
        help="GeoTIFF tiles (highest precision) or 512px thumbnails (fast)",
    )
    parser.add_argument(
        "--cache-dir",
        type=Path,
        default=None,
        help="Download cache (default: assets/wsf2019/tiles or thumbnails)",
    )
    parser.add_argument("--workers", type=int, default=4)
    parser.add_argument(
        "--preset-all-tif",
        action="store_true",
        help="Build wsf2019-{1000,10000,100000}x{N}.bin from GeoTIFF",
    )
    args = parser.parse_args()

    if args.preset_all_tif:
        if args.source != "tif" and args.source is not None:
            pass
        source = "tif"
        cache = args.cache_dir or Path("assets/wsf2019/tiles")
        rc = 0
        for n in (1000, 10000, 100000):
            out = Path(f"assets/masks/wsf2019-{n}x{n}.bin")
            print(f"\n=== {n}x{n} ===", flush=True)
            if build_mask(n, n, out, source, cache, args.workers) != 0:
                rc = 1
        return rc

    if args.width is None or args.height is None or args.out is None:
        parser.error("--width, --height, and --out are required unless --preset-all-tif")
    cache = args.cache_dir
    if cache is None:
        cache = (
            Path("assets/wsf2019/tiles")
            if args.source == "tif"
            else Path("assets/wsf2019/thumbnails")
        )
    return build_mask(args.width, args.height, args.out, args.source, cache, args.workers)


if __name__ == "__main__":
    raise SystemExit(main())
