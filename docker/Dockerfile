# Using inlined version of ubi8-rust-builder, ends at ## END OF ubi8-rust-builder
# Each modification (apart from removing ONBUILD clauses) should be preceded by a ## PATCH comment explaining the change
#FROM docker.stackable.tech/stackable/ubi8-rust-builder AS builder
FROM registry.access.redhat.com/ubi8/ubi-minimal:8.10-1086@sha256:d531ff26bffe832391600e8137f01b9d09a712c721ebe966b98d647bd91bc261 AS builder
LABEL maintainer="Stackable GmbH"

# https://github.com/hadolint/hadolint/wiki/DL4006
SHELL ["/bin/bash", "-o", "pipefail", "-c"]

# Update image and install everything needed for Rustup & Rust
RUN microdnf update --disablerepo=* --enablerepo=ubi-8-appstream-rpms --enablerepo=ubi-8-baseos-rpms -y \
  && rm -rf /var/cache/yum \
  && microdnf install --disablerepo=* --enablerepo=ubi-8-appstream-rpms --enablerepo=ubi-8-baseos-rpms curl findutils gcc gcc-c++ make cmake openssl-devel pkg-config systemd-devel unzip tar xz clang krb5-libs libkadm5 -y \
  && rm -rf /var/cache/yum

WORKDIR /opt/protoc
RUN PROTOC_VERSION=21.5 \
  ARCH=$(arch | sed 's/^aarch64$/aarch_64/') \
  && curl --location --output protoc.zip "https://repo.stackable.tech/repository/packages/protoc/protoc-${PROTOC_VERSION}-linux-${ARCH}.zip" \
  && unzip protoc.zip \
  && rm protoc.zip
ENV PROTOC=/opt/protoc/bin/protoc
WORKDIR /

##PATCH: Install go (for Helm wrapper)
WORKDIR /opt/go
RUN GO_VERSION=1.22.5 \
    ARCH=$(arch | sed 's/^x86_64$/amd64/' | sed 's/^aarch64$/arm64/') \
    && curl --location "https://repo.stackable.tech/repository/packages/go/go${GO_VERSION}.linux-${ARCH}.tar.gz" | tar xvz --strip-components=1 \
    && ln -s /opt/go/bin/go /usr/local/bin
WORKDIR /

##PATCH: Install node (for UI build)
WORKDIR /opt/node
RUN NODE_VERSION=18.16.0 \
    ARCH=$(arch | sed 's/^x86_64$/x64/' | sed 's/^aarch64$/arm64/') \
    && curl --location "https://repo.stackable.tech/repository/packages/node/node-v${NODE_VERSION}-linux-${ARCH}.tar.xz" | tar xvJ --strip-components=1 \
    && ln -s /opt/node/bin/{node,corepack} /usr/local/bin \
    && corepack enable yarn
WORKDIR /

# IMPORTANT
# If you change the toolchain version here, make sure to also change the "rust_version"
# property in operator-templating/repositories.yaml
# hadolint ignore=SC1091
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --default-toolchain 1.68.2
## PATCH: cargo-cyclonedx@0.3.7 uses libcargo 0.64 (Cargo 1.63), while our build uses features from Cargo 1.64
## See https://github.com/CycloneDX/cyclonedx-rust-cargo/discussions/416
#  && . "$HOME/.cargo/env" && cargo install cargo-cyclonedx@0.3.7 cargo-auditable@0.6.1

# Build artifacts will be available in /app.
RUN mkdir /app

## PATCH: docker files are in docker/
COPY docker/ubi8-rust-builder/copy_artifacts.sh /

WORKDIR /src
COPY . /src

# hadolint ignore=SC1091
## PATCH: disabled cyclonedx, run yarn install, build stackable-cockpitd with ui feature enabled
RUN . "$HOME/.cargo/env" && yarn && cargo build --release --workspace --features stackable-cockpitd/ui
# ONBUILD RUN . "$HOME/.cargo/env" && cargo auditable build --release --workspace && cargo cyclonedx --output-pattern package --all --output-cdx

# Copy the "interesting" files into /app.
RUN find /src/target/release \
                -regextype egrep \
                # The interesting binaries are all directly in ${BUILD_DIR}.
                -maxdepth 1 \
                # Well, binaries are executable.
                -executable \
                # Well, binaries are files.
                -type f \
                # Filter out tests.
                ! -regex ".*\-[a-fA-F0-9]{16,16}$" \
                # Copy the matching files into /app.
                -exec /copy_artifacts.sh {} \;

RUN echo "The following files will be copied to the runtime image: $(ls /app)"
## END OF ubi8-rust-builder

FROM registry.access.redhat.com/ubi8/ubi-minimal AS operator

ARG VERSION
ARG RELEASE="1"

LABEL name="Stackable Cockpit" \
  maintainer="info@stackable.de" \
  vendor="Stackable GmbH" \
  version="${VERSION}" \
  release="${RELEASE}" \
  summary="Deploy and manage Stackable clusters." \
  description="Deploy and manage Stackable clusters."

# Update image
RUN microdnf install -y yum \
  && yum -y update-minimal --security --sec-severity=Important --sec-severity=Critical \
  && yum clean all \
  && microdnf clean all

# Install kerberos client libraries
RUN microdnf install -y krb5-libs libkadm5 && microdnf clean all

#COPY LICENSE /licenses/LICENSE

COPY --from=builder /app/* /usr/local/bin/

RUN groupadd -g 1000 stackable && adduser -u 1000 -g stackable -c 'Stackable Operator' stackable

USER stackable:stackable

ENTRYPOINT ["stackable-cockpitd"]
CMD []
