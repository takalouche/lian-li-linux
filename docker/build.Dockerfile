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
    libwebkit2gtk-4.1-dev libgtk-3-dev librsvg2-dev \
    ffmpeg \
  && rm -rf /var/lib/apt/lists/*

ENV PATH=/usr/local/bun/bin:/usr/local/cargo/bin:/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin

RUN curl -fsSL https://bun.sh/install | bash \
  && mv /root/.bun/bin/bun /usr/local/bin/bun \
  && chmod +x /usr/local/bin/bun \
  && bun -v

RUN groupadd -g ${GROUP_ID} builder \
  && useradd -m -u ${USER_ID} -g ${GROUP_ID} -s /bin/bash builder

USER builder
WORKDIR /work

ENV CARGO_TARGET_DIR=/work/target

CMD ["bash", "-c", "\
  set -euo pipefail; \
  set -x; \
  command -v cargo && cargo -V; \
  command -v bun && bun -v; \
  cd /work/crates/lianli-gui && bun install; \
  cd /work && cargo build --release; \
"]
