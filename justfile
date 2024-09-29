dev *ARGS:
    cargo r {{ARGS}}

build:
    cargo b -r --target x86_64-unknown-linux-gnu
    cargo b -r --target x86_64-pc-windows-gnu

fmt:
    wgslfmt src/physics.wgsl
    wgslfmt src/render.wgsl
    cargo fmt

check:
    naga --bulk-validate src/physics.wgsl src/render.wgsl
    cargo clippy
