# Create a single layer image
FROM scratch AS runtime
LABEL org.opencontainers.image.source="https://github.com/dusterthefirst/mcproxy"
LABEL org.opencontainers.image.description="A reverse proxy for your Minecraft: Java Edition servers."
LABEL org.opencontainers.image.licenses="MPL-2.0"

COPY --from=builder /mcproxy /

EXPOSE 25535
ENTRYPOINT ["/mcproxy"]