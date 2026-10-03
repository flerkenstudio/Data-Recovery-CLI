import os
import sys
import json
import subprocess
from pathlib import Path

# Add scripts directory to path for gen_synthetic_ntfs
sys.path.insert(0, str(Path(__file__).parent))
import gen_synthetic_ntfs

def main():
    test_dir = Path("target")
    test_dir.mkdir(exist_ok=True)
    img_path = test_dir / "live_test.img"
    out_dir = test_dir / "live_out"
    cli_bin = Path("target/release/recover-cli.exe")

    print(f"1. Building test NTFS disk image with deleted folder 'MyDataTest' & file 'my_important_doc.txt'...")
    img, meta = gen_synthetic_ntfs.build_image()
    img_path.write_bytes(img)
    print(f"   Created {img_path} ({len(img)} bytes)")

    print(f"\n2. Running CLI scan command...")
    scan_cmd = [str(cli_bin), "scan", str(img_path), "--filter", "MyDataTest"]
    scan_res = subprocess.run(scan_cmd, capture_output=True, text=True)
    print("   STDOUT:")
    print(scan_res.stdout)

    if scan_res.returncode != 0:
        print("   STDERR:")
        print(scan_res.stderr)
        sys.exit(1)

    print(f"\n3. Running CLI recover command...")
    rec_cmd = [str(cli_bin), "recover", str(img_path), "--output", str(out_dir), "--filter", "MyDataTest"]
    rec_res = subprocess.run(rec_cmd, capture_output=True, text=True)
    print("   STDOUT:")
    print(rec_res.stdout)

    if rec_res.returncode != 0:
        print("   STDERR:")
        print(rec_res.stderr)
        sys.exit(1)

    print(f"\n4. Verifying recovered file content...")
    recovered_files = list(out_dir.glob("*"))
    print(f"   Recovered files in {out_dir}: {[f.name for f in recovered_files]}")

    for f in recovered_files:
        content = f.read_bytes()
        print(f"   File '{f.name}' size: {len(content)} bytes")
        print(f"   File '{f.name}' content: {content!r}")

    print("\nSUCCESS: End-to-end recovery test completed successfully!")

if __name__ == "__main__":
    main()
