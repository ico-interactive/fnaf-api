# defaults
ARG RUST_VER=1.96.0
ARG APP_NAME=fnaf-api
ARG UID=10001

FROM rust:${RUST_VER}-alpine3.24 AS fetch_apk
WORKDIR /app
RUN --mount=type=cache,target=/var/cache/apk \
    apk add --update-cache clang lld musl-dev git

FROM fetch_apk AS fetch_cargo
COPY Cargo.toml Cargo.lock .
RUN mkdir src; touch src/main.rs
RUN --mount=type=cache,target=/usr/local/cargo/git/db \   
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    cargo fetch --locked

# build
FROM fetch_cargo AS build
ARG APP_NAME
COPY . .
RUN --mount=type=cache,target=/app/target/ \
    --mount=type=cache,target=/usr/local/cargo/registry/ \
    --mount=type=cache,target=/usr/local/cargo/git/db \   
    cargo build --frozen --release && \
    cp "./target/release/$APP_NAME" /bin/server

# setup non privileged user
FROM alpine:3.24.1 AS setup
ARG UID
RUN adduser \
    --disabled-password \
    --gecos "" \
    --home "/nonexistent" \
    --shell "/sbin/nologin" \
    --no-create-home \
    --uid "${UID}" \
    appuser
USER appuser
COPY --from=build /bin/server /bin

# run
FROM setup AS prod
EXPOSE 9638
CMD ["server"]

