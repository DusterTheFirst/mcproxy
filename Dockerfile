# Create a single layer image
FROM scratch AS runtime

ADD ./mcproxy /bin/

# TODO: healthcheck
# What health is there to check? panics would bring the whole container down. Can the
# program be unhealthy and still run?
# HEALTHCHECK --interval=30s --timeout=30s --start-period=5s --retries=3 CMD [ "executable" ]

EXPOSE 25565
ENTRYPOINT ["/bin/mcproxy"]