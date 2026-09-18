set shell := ["powershell.exe", "-NoProfile", "-Command"]

# list available recipes
default:
    @just --list

# release build (default FemtoVG renderer)
build:
    cargo build --release

# run in debug; pass cargo flags as one quoted string
# e.g. just run "--no-default-features --features skia"
run *args:
    cargo run {{ args }}

# remove build artifacts: ./target + skia/wgpu benchmark target dirs
clean:
    cargo clean
    Remove-Item -Recurse -Force target-skia, target-wgpu -ErrorAction SilentlyContinue
