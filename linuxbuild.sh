#!/bin/bash
cargo build --release
cargo build --release --target i686-unknown-linux-gnu
cargo build --release --target aarch64-linux-android 