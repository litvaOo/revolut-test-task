FROM rust:1.79.0
RUN cargo install cargo-watch
WORKDIR /service
COPY . .
CMD cargo watch -x 'run'


