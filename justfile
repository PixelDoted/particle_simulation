set export
alias r := run
alias b := build
alias ga := git-add

run *ARGS:
    cargo r {{ARGS}}

build:
    cargo b -r

fmt:
    wgslfmt src/collision.wgsl
    wgslfmt src/gravity.wgsl
    wgslfmt src/render.wgsl
    cargo fmt

check:
    naga --bulk-validate src/collision.wgsl src/gravity.wgsl src/render.wgsl
    cargo clippy

git-add: check fmt
    git add *
