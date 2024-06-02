FROM ubuntu:22.04
EXPOSE 8080
WORKDIR /app
COPY ./static ./static
COPY ./bsky-quote-explorer /bin/bsky-quote-explorer
CMD ["/bin/bsky-quote-explorer"]
