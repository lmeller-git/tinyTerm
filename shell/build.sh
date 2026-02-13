#! /bin/bash

echo "building tinyShell in $(pwd)"

cargo build --release --target target.json -Zjson-target-spec
# ln -sf ./target/target/release/tinyShell a.out

echo "tinyShell built"
