set export
alias r := run
alias b := build

run *ARGS:
    cargo r {{ARGS}}

build:
    cargo b -r

fmt:
    wgslfmt src/physics.wgsl
    wgslfmt src/render.wgsl
    cargo fmt

check:
    naga --bulk-validate src/physics.wgsl src/render.wgsl
    cargo clippy
