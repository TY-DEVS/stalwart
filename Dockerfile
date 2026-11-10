ARG STALWART_VERSION=v1.1.0

FROM docker.io/debian:trixie-slim

ARG STALWART_VERSION
ARG TARGETARCH

RUN export DEBIAN_FRONTEND=noninteractive && \
    apt-get update && \
    apt-get install -yq --no-install-recommends ca-certificates curl tzdata libcap2-bin tar gzip && \
    rm -rf /var/lib/apt/lists/* && \
    groupadd -r -g 2000 stalwart && \
    useradd -r -u 2000 -g 2000 -s /usr/sbin/nologin -M stalwart && \
    mkdir -p /etc/stalwart /var/lib/stalwart && \
    chown stalwart:stalwart /etc/stalwart /var/lib/stalwart

# Install Stalwart Mail Server binary
RUN case "${TARGETARCH:-$(uname -m)}" in \
        x86_64|amd64) ARCH="x86_64-unknown-linux-gnu" ;; \
        aarch64|arm64) ARCH="aarch64-unknown-linux-gnu" ;; \
        *) ARCH="x86_64-unknown-linux-gnu" ;; \
    esac && \
    curl -fsSL "https://github.com/stalwartlabs/stalwart/releases/download/${STALWART_VERSION}/stalwart-${ARCH}.tar.gz" -o /tmp/stalwart.tar.gz && \
    tar -xzf /tmp/stalwart.tar.gz -C /usr/local/bin && \
    rm -f /tmp/stalwart.tar.gz && \
    chmod 0755 /usr/local/bin/stalwart && \
    setcap 'cap_net_bind_service=+ep' /usr/local/bin/stalwart

USER stalwart
WORKDIR /var/lib/stalwart
VOLUME ["/etc/stalwart", "/var/lib/stalwart"]
EXPOSE 25 110 143 465 587 993 995 4190 8080

ENV STALWART_HEALTHCHECK_URL=http://127.0.0.1:8080/healthz/live
HEALTHCHECK --interval=30s --timeout=5s --start-period=30s --retries=3 \
    CMD curl -fsS "$STALWART_HEALTHCHECK_URL" || exit 1

ENTRYPOINT ["/usr/local/bin/stalwart"]
CMD ["--config", "/etc/stalwart/config.json"]
