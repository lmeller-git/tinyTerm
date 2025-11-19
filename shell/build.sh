#! /bin/bash

echo "building tinyShell in $(pwd)"

cargo build --release
# ln -sf ./target/target/release/tinyShell a.out

echo "tinyShell built"
