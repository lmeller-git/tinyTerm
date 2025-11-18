#! /bin/bash

echo "building tinyTerm in $(pwd)"

cargo build --release
ln -sf a.out ./target/target/release/tinyTerm

echo "tinyTerm built"
