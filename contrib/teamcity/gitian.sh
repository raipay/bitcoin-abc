#!/usr/bin/env bash

export LC_ALL=C

set -euxo pipefail

: "${TOPLEVEL:=$(git rev-parse --show-toplevel)}"
: "${BUILD_DIR:=${TOPLEVEL}/build}"
: "${THREADS:=$(nproc || sysctl -n hw.ncpu)}"

COMMIT=$(git -C "${TOPLEVEL}" rev-parse HEAD)
export COMMIT
export USE_DOCKER=1

cd "${TOPLEVEL}/contrib/gitian-builder"

./bin/make-base-vm --docker --arch amd64 --distro debian --suite bullseye

if [[ "${OS_NAME}" == "osx" ]]; then
  OSX_SDK_DIR=~/.abc-build-cache/osx-sdk
  mkdir -p "${OSX_SDK_DIR}"

  OSX_SDK=$("${TOPLEVEL}/contrib/teamcity/download-apple-sdk.sh" "${OSX_SDK_DIR}")

  mkdir -p inputs
  cp "${OSX_SDK_DIR}/${OSX_SDK}" inputs/"${OSX_SDK}"
fi

RESULT_DIR="${BUILD_DIR}/gitian-results"
OS_DIR="${RESULT_DIR}/${OS_NAME}"
mkdir -p "${OS_DIR}"

move_log() {
  mv var/install.log "${RESULT_DIR}/"
  mv var/build.log "${RESULT_DIR}/"
}
trap "move_log" ERR

./bin/gbuild -j${THREADS} -m3500 --commit bitcoin=${COMMIT} --url bitcoin="${TOPLEVEL}" "${TOPLEVEL}/contrib/gitian-descriptors/gitian-${OS_NAME}.yml"

move_log
mv result/*.yml "${OS_DIR}/"
mv build/out/* "${OS_DIR}/"
