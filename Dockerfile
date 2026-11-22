# The server in the bundle `dx bundle --package web --platform web --release`
# writes links glibc, so build it where glibc is no newer than Debian 13's 2.41.
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:cb87acc574abe719b6e5c547384defa2f7b1f24af9605d3f618cf852495d8be2
WORKDIR /app
COPY target/dx/web/release/web/ ./
CMD ["/app/server"]
