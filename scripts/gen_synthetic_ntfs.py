#!/usr/bin/env python3
"""
Generate a minimal synthetic NTFS-like disk image for parser/recovery unit tests.

This is NOT a full NTFS volume. It is carefully shaped so that:
  - Boot sector OEM = "NTFS    ", 0x55AA
  - $MFT starts at a known LCN with contiguous records
  - Several FILE records exist (in-use dirs + deleted files)
  - One deleted file has resident data
  - One deleted file has non-resident data runs pointing at real clusters

Layout (4 KiB clusters, 1 KiB MFT records):
  sector 0          : boot
  cluster 4         : MFT start (records 0..)
  cluster 100       : non-resident file payload ("HELLO-NONRESIDENT-PAYLOAD")
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import struct
from pathlib import Path

SECTOR = 512
SPC = 8  # sectors per cluster → 4096-byte clusters
BPC = SECTOR * SPC
RECORD = 1024
MFT_LCN = 4
MFT_OFFSET = MFT_LCN * BPC
PAYLOAD_LCN = 100
PAYLOAD_OFFSET = PAYLOAD_LCN * BPC
IMAGE_CLUSTERS = 128
IMAGE_SIZE = IMAGE_CLUSTERS * BPC


def utf16le(s: str) -> bytes:
    return s.encode("utf-16le")


def put_u16(buf: bytearray, off: int, v: int) -> None:
    struct.pack_into("<H", buf, off, v & 0xFFFF)


def put_u32(buf: bytearray, off: int, v: int) -> None:
    struct.pack_into("<I", buf, off, v & 0xFFFFFFFF)


def put_u64(buf: bytearray, off: int, v: int) -> None:
    struct.pack_into("<Q", buf, off, v & 0xFFFFFFFFFFFFFFFF)


def build_boot() -> bytes:
    data = bytearray(SECTOR)
    data[0] = 0xEB
    data[1] = 0x52
    data[2] = 0x90
    data[3:11] = b"NTFS    "
    put_u16(data, 0x0B, SECTOR)
    data[0x0D] = SPC
    put_u64(data, 0x28, IMAGE_SIZE // SECTOR)  # total sectors
    put_u64(data, 0x30, MFT_LCN)
    put_u64(data, 0x38, MFT_LCN + 1)  # mirror (dummy)
    data[0x40] = 0xF6  # -10 → 1024-byte FILE records
    data[0x44] = 0x01
    put_u64(data, 0x48, 0x0123456789ABCDEF)
    data[0x1FE] = 0x55
    data[0x1FF] = 0xAA
    return bytes(data)


def encode_runlist(lcn: int, clusters: int) -> bytes:
    """Single run: 1-byte length, 1-byte offset (relative from 0)."""
    # header 0x11, length, offset
    return bytes([0x11, clusters & 0xFF, lcn & 0xFF, 0x00])


def build_file_record(
    record_number: int,
    *,
    in_use: bool,
    is_dir: bool,
    name: str,
    parent_ref: int,
    resident_data: bytes | None = None,
    nonresident: tuple[int, int, int] | None = None,
    # nonresident = (lcn, cluster_count, data_size)
) -> bytes:
    buf = bytearray(RECORD)
    buf[0:4] = b"FILE"
    put_u16(buf, 4, 0x30)  # usa offset
    put_u16(buf, 6, 1)  # usa count (USN only — tests skip strict USA)
    put_u16(buf, 16, 1)  # sequence
    put_u16(buf, 18, 1)  # link count
    put_u16(buf, 20, 0x38)  # attrs offset
    flags = 0
    if in_use:
        flags |= 0x0001
    if is_dir:
        flags |= 0x0002
    put_u16(buf, 22, flags)

    pos = 0x38

    # --- STANDARD_INFORMATION (resident, type 0x10) ---
    si_val = bytearray(48)
    # timestamps left zero
    put_u32(si_val, 32, 0x20 if not is_dir else 0x10)  # ARCHIVE / DIRECTORY-ish
    si_attr = bytearray(24 + len(si_val))
    put_u32(si_attr, 0, 0x10)
    put_u32(si_attr, 4, len(si_attr))
    si_attr[8] = 0  # resident
    put_u32(si_attr, 16, len(si_val))
    put_u16(si_attr, 20, 24)
    si_attr[24:] = si_val
    buf[pos : pos + len(si_attr)] = si_attr
    pos += len(si_attr)

    # --- FILE_NAME (resident, type 0x30) ---
    name_u = utf16le(name)
    fn_val = bytearray(66 + len(name_u))
    put_u64(fn_val, 0, parent_ref & 0xFFFFFFFFFFFF)
    fn_val[64] = len(name)
    fn_val[65] = 1  # Win32
    fn_val[66 : 66 + len(name_u)] = name_u
    fn_attr = bytearray(24 + len(fn_val))
    put_u32(fn_attr, 0, 0x30)
    put_u32(fn_attr, 4, len(fn_attr))
    fn_attr[8] = 0
    put_u32(fn_attr, 16, len(fn_val))
    put_u16(fn_attr, 20, 24)
    fn_attr[24:] = fn_val
    buf[pos : pos + len(fn_attr)] = fn_attr
    pos += len(fn_attr)

    # --- DATA ---
    if resident_data is not None:
        da = bytearray(24 + len(resident_data))
        put_u32(da, 0, 0x80)
        put_u32(da, 4, len(da))
        da[8] = 0
        put_u32(da, 16, len(resident_data))
        put_u16(da, 20, 24)
        da[24:] = resident_data
        buf[pos : pos + len(da)] = da
        pos += len(da)
    elif nonresident is not None:
        lcn, clusters, data_size = nonresident
        run = encode_runlist(lcn, clusters)
        # Non-resident header is at least 0x40 bytes; runlist after
        header_size = 0x40
        da = bytearray(header_size + len(run))
        put_u32(da, 0, 0x80)
        put_u32(da, 4, len(da))
        da[8] = 1  # non-resident
        put_u16(da, 32, header_size)  # runlist offset
        put_u64(da, 40, clusters * BPC)  # allocated
        put_u64(da, 48, data_size)  # real size
        put_u64(da, 56, data_size)  # initialized
        da[header_size:] = run
        buf[pos : pos + len(da)] = da
        pos += len(da)

    # End marker
    put_u32(buf, pos, 0xFFFFFFFF)
    pos += 4

    put_u32(buf, 24, pos)  # used size
    put_u32(buf, 28, RECORD)  # allocated

    # Store record number in a conventional place (not required by our parser)
    _ = record_number
    return bytes(buf)


def build_image() -> tuple[bytes, dict]:
    img = bytearray(IMAGE_SIZE)
    img[0:SECTOR] = build_boot()

    # Payload for non-resident deleted file
    payload = b"HELLO-NONRESIDENT-PAYLOAD"
    img[PAYLOAD_OFFSET : PAYLOAD_OFFSET + len(payload)] = payload

    resident_payload = b"hello resident deleted file\n"
    meta = {
        "bytes_per_cluster": BPC,
        "mft_lcn": MFT_LCN,
        "record_size": RECORD,
        "files": [],
    }

    records: list[bytes] = []

    # 0: $MFT — non-resident covering first N records in cluster 4
    # We put enough clusters so sequential scan works; data_size = n * RECORD
    mft_clusters = 3  # 3 * 4096 = 12 records capacity
    mft_data_size = 12 * RECORD
    records.append(
        build_file_record(
            0,
            in_use=True,
            is_dir=False,
            name="$MFT",
            parent_ref=5,
            nonresident=(MFT_LCN, mft_clusters, mft_data_size),
        )
    )

    # 1–4: placeholders (in-use system-ish)
    for i in range(1, 5):
        records.append(
            build_file_record(
                i,
                in_use=True,
                is_dir=False,
                name=f"$Sys{i}",
                parent_ref=5,
                resident_data=b"",
            )
        )

    # 5: root directory
    records.append(
        build_file_record(
            5,
            in_use=True,
            is_dir=True,
            name=".",
            parent_ref=5,
        )
    )

    # 6: Documents (directory)
    records.append(
        build_file_record(
            6,
            in_use=True,
            is_dir=True,
            name="Documents",
            parent_ref=5,
        )
    )

    # 7: deleted resident file under Documents
    records.append(
        build_file_record(
            7,
            in_use=False,
            is_dir=False,
            name="notes.txt",
            parent_ref=6,
            resident_data=resident_payload,
        )
    )
    meta["files"].append(
        {
            "record": 7,
            "name": "notes.txt",
            "path": "Documents/notes.txt",
            "deleted": True,
            "sha256": hashlib.sha256(resident_payload).hexdigest(),
            "size": len(resident_payload),
            "kind": "resident",
        }
    )

    # 8: deleted non-resident file under root
    records.append(
        build_file_record(
            8,
            in_use=False,
            is_dir=False,
            name="payload.bin",
            parent_ref=5,
            nonresident=(PAYLOAD_LCN, 1, len(payload)),
        )
    )
    meta["files"].append(
        {
            "record": 8,
            "name": "payload.bin",
            "path": "payload.bin",
            "deleted": True,
            "sha256": hashlib.sha256(payload).hexdigest(),
            "size": len(payload),
            "kind": "nonresident",
        }
    )

    # Write MFT records at MFT_OFFSET
    for i, rec in enumerate(records):
        off = MFT_OFFSET + i * RECORD
        img[off : off + RECORD] = rec

    return bytes(img), meta


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "-o",
        "--output",
        type=Path,
        default=Path(__file__).resolve().parents[1]
        / "tests"
        / "fixtures"
        / "synthetic_ntfs.img",
    )
    args = parser.parse_args()
    args.output.parent.mkdir(parents=True, exist_ok=True)
    img, meta = build_image()
    args.output.write_bytes(img)
    meta_path = args.output.with_suffix(".json")
    meta_path.write_text(json.dumps(meta, indent=2))
    print(f"Wrote {args.output} ({len(img)} bytes)")
    print(f"Wrote {meta_path}")
    for f in meta["files"]:
        print(f"  record {f['record']}: {f['path']} sha256={f['sha256'][:16]}...")


if __name__ == "__main__":
    main()
