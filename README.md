# tinyTerm

The default terminal emulator and userspace shell for tinyOS.

![Screenshot of tinyTerm + tinyShell running on tinyos](https://github.com/lmeller-git/tinyos/raw/main/assets/in_usage.png)

## Overview

`tinyTerm` and `tinyShell` together form the default user interface for **[tinyOS](https://github.com/lmeller-git/tinyos)**. This repository contains two core userspace components:

1. **The Terminal Backend:** A graphical terminal emulator that communicates directly with tinyOS's framebuffers and I/O streams.
2. **The Default Shell:** An interactive command-line interface for user input and program execution.

To preserve modularity, both the terminal backend and the user shell are decoupled and either implementation can be independently swapped out for a custom alternative.

> **Note on Interoperability:** If only one of the two components is replaced, the specific initialization sequences implemented in `shell/src/init.rs` and `term/src/init.rs` should be respected to ensure proper environment setup and I/O stream piping.

---

## Technical Composition

Built specifically for the tinyOS userspace ecosystem, `tinyTerm` leverages:
* **`libtinyos`**: Handles standard input/output streams and interactions via the native `tinyos_abi`.
* **`libtinygraphics`**: Manages the underlying window layers and exposed framebuffers.
* **Ratatui** + **mousefood**: Empowers the terminal with a rich, ergonomic, and responsive text-based UI framework.
* **vte**: Provides canonical, robust parsing and handling of standard ANSI escape sequences.

---

## How to Build & Run

This repository is designed to be compiled alongside the primary tinyOS image and currently only supports tinyOS as target.

The easiest way to test or modify `tinyTerm` is to build it within the main tinyOS build system. Instructions to do so may be found in **[tinyOS README](https://github.com/lmeller-git/tinyos/blob/main/README.md)**.
