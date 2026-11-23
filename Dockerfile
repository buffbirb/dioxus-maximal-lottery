# The server in the bundle `dx bundle --package web --platform web --release`
# writes links glibc, so build it where glibc is no newer than Debian 13's 2.41.
FROM gcr.io/distroless/cc-debian13:nonroot@sha256:532c7a94f9e96ba9db75eb9388f4227751131c7cb0a411d1a8521af4096135de
WORKDIR /app
COPY target/dx/web/release/web/ ./
CMD ["/app/server"]
