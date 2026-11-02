FROM rust:1-trixie@sha256:931c2c0531a8b143e044f0f3914dfe802f15bb3d1955f0b657e9359327ac2ffc AS builder
WORKDIR /build
COPY . .
# target-cpu=haswell: enables AVX2 on i5-13500 P-cores (no AVX-512 on this machine)
# fat LTO + codegen-units=1 already set in Cargo.toml profile.release
RUN RUSTFLAGS="-C target-cpu=haswell" cargo build --release

FROM debian:13-slim@sha256:62bd423aeee912639ca8ee681c2b762cb3e25137e0a92b20be455898a9f034ad
COPY --from=builder /build/target/release/track-c /usr/local/bin/track-c
CMD ["/usr/local/bin/track-c"]
