#! /bin/bash

echo "building tinyTerm in $(pwd)"

cargo build --release
# ln -sf ./target/target/release/tinyTerm a.out

echo "tinyTerm built"
