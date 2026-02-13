#! /bin/bash

echo "building tinyTerm in $(pwd)"

cargo build --release --target target.json -Zjson-target-spec
ln -sf ./target/target/release/tinyTerm a.out

echo "tinyTerm built"
