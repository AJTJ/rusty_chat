FROM rust as builder

RUN cargo build --release

FROM bitnami/minideb as runner
COPY --from=builder /app/target/release/app /app
CMD /app