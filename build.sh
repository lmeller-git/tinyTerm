#! /bin/bash

# this buld script should not be used manually. It exists only for interface with tinyosprograms - tinyOS setup

echo "building tinyTerm in $(pwd)..."

cargo build --release

echo "tinyTerm built"
echo "creating symlinks to shell and term in top level dirs..."

ln -sf ./term/a.out a.out

if [ ! -d ../tinyShell ]; then
  mkdir ../tinyShell
fi
cd ../tinyShell
ln -sf ../tinyTerm/shell/a.out a.out

