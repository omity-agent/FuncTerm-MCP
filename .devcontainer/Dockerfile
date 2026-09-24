# syntax=docker/dockerfile:1
ARG RUST_IMAGE=rustlang/rust:nightly-bookworm
ARG BUN_IMAGE=oven/bun:1

FROM ${BUN_IMAGE} AS bun
FROM ${RUST_IMAGE}

ARG DEVELOPER_UID=1000
ARG DEVELOPER_GID=1000

RUN <<EOF
set -o errexit
export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install --yes --no-install-recommends gdb python3 ripgrep zsh
rm --recursive --force /var/lib/apt/lists/*
rustup component add clippy rustfmt rust-src
EOF

COPY --from=bun /usr/local/bin/bun /usr/local/bin/bun

ENV CARGO_HOME=/cache/cargo \
    CARGO_TARGET_DIR=/cache/build \
    PATH=/cache/cargo/bin:${PATH} \
    LANG=C.UTF-8 \
    TERM=xterm-256color

# 预先设置挂载点的所有者，让新建的缓存卷可由普通用户写入。
RUN <<EOF
set -o errexit
groupadd --gid "${DEVELOPER_GID}" developer
useradd --uid "${DEVELOPER_UID}" --gid "${DEVELOPER_GID}" --create-home --shell /bin/bash developer
mkdir --parents "${CARGO_HOME}" "${CARGO_TARGET_DIR}" /workspaces/FuncTerm
chown --recursive developer:developer /cache /workspaces/FuncTerm
EOF

USER developer
WORKDIR /workspaces/FuncTerm
CMD ["sleep", "infinity"]
