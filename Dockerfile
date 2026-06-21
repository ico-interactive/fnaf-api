ARG RUST_VER=1.96.0
ARG APP_NAME=fnaf-api

# build
FROM rust:${RUST_VER}-alpine3.24 AS build

ARG APP_NAME

WORKDIR /app

RUN apk add --no-cache clang lld musl-dev git

RUN --mount=type=bind,source=src,target=src \
    --mount=type=bind,source=Cargo.toml,target=Cargo.toml \
    --mount=type=bind,source=Cargo.lock,target=Cargo.lock \
    --mount=type=bind,source=NotoSerifDisplay.otf,target=NotoSerifDisplay.otf \
    --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo build --locked --release && \
    cp "./target/release/$APP_NAME" /bin/server

# run
FROM alpine:3.24.1 AS final
ARG APP_NAME

# non privileged user
ARG UID=10001
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser

# enter user
USER appuser

COPY --from=build /bin/server /bin

EXPOSE 9638

CMD ["server"]
