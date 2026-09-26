#!/usr/bin/env bash
# Full verification script executed inside WSL/Linux: cargo test + PTY smoke tests
set -e
export PATH="$HOME/.rust-linux/bin:$PATH"

echo "== rust =="; cargo --version

# The project is copied to a WSL-local directory for building (building on /mnt/d is too slow)
rm -rf ~/cctui && mkdir -p ~/cctui
cp -r /mnt/d/Dev_Project/C/Cargo.toml /mnt/d/Dev_Project/C/src /mnt/d/Dev_Project/C/examples /mnt/d/Dev_Project/C/tests /mnt/d/Dev_Project/C/tools /mnt/d/Dev_Project/C/README.md ~/cctui/

cd ~/cctui
echo "== cargo test (native Linux) =="
cargo test 2>&1 | grep -E "test result|running [0-9]+ test|^error" || true

echo "== PTY smoke tests =="
if [ $# -ge 1 ]; then
    python3 tools/pty_smoke.py "$1"
else
    python3 tools/pty_smoke.py
fi
