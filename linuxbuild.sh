#!/bin/bash
unset CC
mkdir target/build
cargo build --release
mv target/release/libgodot_voice.so target/build/libgodot_voice-linux64.so
cargo build --release --target i686-unknown-linux-gnu
mv target/i686-unknown-linux-gnu/release/libgodot_voice.so target/build/libgodot_voice-linux32.so
export CC=$HOME/Android/Sdk/ndk/21.4.7075529/toolchains/llvm/prebuilt/linux-x86_64/bin/aarch64-linux-android21-clang
cargo build --release --target aarch64-linux-android 
mv target/aarch64-linux-android/release/libgodot_voice.so target/build/libgodot_voice-android-arm64.so 
export CC=$HOME/Android/Sdk/ndk/21.4.7075529/toolchains/llvm/prebuilt/linux-x86_64/bin/armv7a-linux-androideabi21-clang
cargo build --release --target armv7-linux-androideabi
mv target/armv7-linux-androideabi/release/libgodot_voice.so target/build/libgodot_voice-android-arm32.so 