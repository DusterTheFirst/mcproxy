#!/bin/sh

set -eux

if [ "$#" -gt 0 ]; then
    target=$1
else
    target="armv7-unknown-linux-musleabihf"
fi

if [ "$#" -gt 1 ]; then
    features=$2
else
    features="pid1,metrics"
fi

cd "$(dirname "$0")"

cross build --release --target="$target" --features="$features"

mkdir -p ./target/docker
ln -f ./target/$target/release/mcproxy ./target/docker/mcproxy

docker buildx build ./target/docker -f Dockerfile -t ghcr.io/dusterthefirst/mcproxy:alpha