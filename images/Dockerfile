FROM rust:latest as build

## Install dependencies
RUN apt-get update \
    && apt-get install -y \
        build-essential \
        clang \
        cmake \
        git \
        libboost-all-dev \
        libevent-dev \
        libssl-dev \
        llvm \
        ninja-build \
        protobuf-compiler \
        wget \
    && rustup component add rustfmt

## Build Corrosion
RUN wget https://api.github.com/repos/corrosion-rs/corrosion/tarball/v0.3.0 -O corrosion.tar.gz \
    && echo "3b9a48737264add649983df26c83f3285ce17e20d86194c7756689a0d8470267 corrosion.tar.gz" | sha256sum -c \
    && tar xzf corrosion.tar.gz \
    && CORROSION_SRC_FOLDER=corrosion-rs-corrosion-b764a9f \
    && CORROSION_BUILD_FOLDER=${CORROSION_SRC_FOLDER}-build \
    && cmake -S${CORROSION_SRC_FOLDER} -B${CORROSION_BUILD_FOLDER} -DCMAKE_BUILD_TYPE=Release \
    && cmake --build ${CORROSION_BUILD_FOLDER} --config Release \
    && cmake --install ${CORROSION_BUILD_FOLDER} --config Release

## Build the node
## Copy bitcoin-abc executables into /bitcoin-abc/bin
RUN mkdir /bitcoin-abc-src
COPY . /bitcoin-abc-src
RUN mkdir -p /build \
    && mkdir -p /bitcoin-abc/bin/ \
    && cd /build \
    && cmake -GNinja \
        -DBUILD_BITCOIN_CHRONIK=on \
        -DUSE_JEMALLOC=off \
        -DBUILD_BITCOIN_QT=off \
        -DBUILD_BITCOIN_WALLET=off \
        -DBUILD_BITCOIN_ZMQ=off \
        -DENABLE_NATPMP=off \
        -DENABLE_UPNP=off \
        /bitcoin-abc-src \
    && ninja \
    && cp /build/src/bitcoin* /bitcoin-abc/bin/ \
    && rm -r /build
