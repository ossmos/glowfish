# Installation

glowfish is built from source with the Rust toolchain.

## Requirements

- A graphics card with **Vulkan** (Linux, Windows), DirectX 12 (Windows) or Metal (macOS)
  support. Exports render on the graphics card as well, so they also need a GPU; they do not
  need a window or a display.
- Rust 1.97.1 or newer, installed with [rustup](https://rustup.rs).
- On Linux, the development packages Bevy needs:

    === "Debian / Ubuntu"

        ```sh
        sudo apt install pkg-config libasound2-dev libudev-dev libwayland-dev libxkbcommon-dev libx11-dev
        ```

    === "Arch Linux"

        ```sh
        sudo pacman -S pkgconf alsa-lib systemd-libs wayland libxkbcommon libx11
        ```

## Building

From the repository root:

```sh
cargo install --path crates/cli
```

This builds an optimized `glowfish` binary and installs it into `~/.cargo/bin`. Check the
installation with:

```sh
glowfish --version
```

To open measurement files from your file manager with a right click, see
[Opening from the file manager](usage/file-manager.md).

!!! tip "Older hardware"
    glowfish adapts how much it draws through a single setting, `view.max_cells`. If the
    viewer feels slow, lower it, see [Configuration](configuration.md#older-hardware).
