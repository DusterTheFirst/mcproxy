# Create a single layer image
FROM scratch AS runtime

ADD ./mcproxy /

EXPOSE 25535
ENTRYPOINT ["/mcproxy"]