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

# local CI replacement (the GitHub workflow was removed on purpose):
# everything a push would run, before you commit
check:
    cargo check --all-targets
    cargo test
    cargo build --release

# headless visual shot: software-rendered PNG of the real UI, no window.
# Scene names: default dark palette search-notes menu rename settings dialog empty
shot scene="default":
    cargo build --features software --bin quire-shot
    .\target\debug\quire-shot.exe --out .scratch\shots\latest.bmp --scene {{ scene }}
    powershell -NoProfile -ExecutionPolicy Bypass -File benchmarks\scripts\shot2png.ps1

# package the release exe into a zip (M8-lite; installer comes later)
dist:
    cargo build --release
    powershell -NoProfile -Command       "New-Item -ItemType Directory -Force dist | Out-Null;        Copy-Item .	argetelease\quire.exe .\dist\quire.exe -Force;        Compress-Archive -Path .\dist\quire.exe -DestinationPath .\dist\quire-windows-x64.zip -Force;        Remove-Item .\dist\quire.exe; Write-Output 'dist/quire-windows-x64.zip ready'"

# remove build artifacts: ./target + skia/wgpu benchmark target dirs
clean:
    cargo clean
    Remove-Item -Recurse -Force target-skia, target-wgpu -ErrorAction SilentlyContinue
