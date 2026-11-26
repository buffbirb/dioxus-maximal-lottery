# The server in the bundle `dx bundle --package web --platform web --release`
# writes links glibc, so build it where glibc is no newer than Debian 13's 2.41.
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:29ee70762214fe8190cea04ef1c0a84a8365879e047ce8f911ae5a5ab2355fa2
WORKDIR /app
COPY target/dx/web/release/web/ ./
CMD ["/app/server"]
