FROM rust:1.93-slim
RUN apt-get update && apt-get --assume-yes install libudev-dev
RUN mkdir -p /opt/
RUN rustup install 1.93.0
VOLUME /opt
ENV APP_NAME=${TARGET_APP}
CMD ["sh","-c","cd /opt/${APP_NAME} && cargo build --release --target x86_64-unknown-linux-gnu"]
