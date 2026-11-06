FROM mcr.microsoft.com/devcontainers/base:ubuntu

ARG JUST_VERSION=1.59.0

RUN set -eux; \
    case "$(dpkg --print-architecture)" in \
      amd64) just_target="x86_64-unknown-linux-musl" ;; \
      arm64) just_target="aarch64-unknown-linux-musl" ;; \
      *) echo "unsupported architecture: $(dpkg --print-architecture)" >&2; exit 1 ;; \
    esac; \
    archive="just-${JUST_VERSION}-${just_target}.tar.gz"; \
    release_url="https://github.com/casey/just/releases/download/${JUST_VERSION}"; \
    work_dir="$(mktemp -d)"; \
    curl --fail --location --silent --show-error \
      --output "${work_dir}/${archive}" \
      "${release_url}/${archive}"; \
    curl --fail --location --silent --show-error \
      --output "${work_dir}/SHA256SUMS" \
      "${release_url}/SHA256SUMS"; \
    cd "$work_dir"; \
    grep "  ${archive}$" SHA256SUMS | sha256sum --check --strict -; \
    tar --extract --gzip --file "$archive" just; \
    install -m 0755 just /usr/local/bin/just; \
    rm -rf "$work_dir"
