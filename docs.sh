#!/bin/bash
export RUSTFLAGS="-C target-feature=aes,sse2"
exec cargo doc --target=x86_64-unknown-linux-gnu $@
