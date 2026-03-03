FROM rust:1.93-trixie

ARG DEBIAN_FRONTEND=noninteractive
ARG USER_ID=1000
ARG GROUP_ID=1000

RUN apt-get update && apt-get install -y --no-install-recommends \
    ca-certificates curl git \
    build-essential pkg-config \
    clang cmake ninja-build \
    libssl-dev \
    libhidapi-dev libusb-1.0-0-dev libudev-dev \
    ffmpeg \
  && rm -rf /var/lib/apt/lists/*

RUN groupadd -g ${GROUP_ID} builder \
  && useradd -m -u ${USER_ID} -g ${GROUP_ID} -s /bin/bash builder

USER builder
WORKDIR /work

ENV CARGO_TARGET_DIR=/work/target

CMD ["bash", "-c", "\
  set -euo pipefail; \
  set -x; \
  command -v cargo && cargo -V; \
  cd /work && cargo build --release; \
"]
