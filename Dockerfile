FROM scratch
COPY target/x86_64-unknown-linux-musl/release/pwd-interface /bin/pwd-interface
ENV PORT=80
ENV BASE_PATH=/srv
WORKDIR /srv
ENTRYPOINT ["/bin/pwd-interface"]
