FROM busybox:musl
COPY target/x86_64-unknown-linux-musl/release/pwdi-server /bin/pwdi-server
ENV PORT=80
WORKDIR /srv
ENTRYPOINT ["/bin/pwdi-server"]
