
VERSION 0.6
FROM rust:1.67.1-slim-buster # make sure to update rust-toolchain.toml too so that everything uses the same rust version
WORKDIR /earthbuild

prep-debian:
    RUN apt -y update

install-other-libs:
    FROM +prep-debian
    RUN apt -y install wget git
    RUN apt -y install libunwind-dev pkg-config zlib1g-dev
    RUN apt -y install unzip # for www/build.sh

install-zig-llvm:
    FROM +install-other-libs
    # zig
    RUN wget -c https://ziglang.org/download/0.9.1/zig-linux-x86_64-0.9.1.tar.xz --no-check-certificate
    RUN tar -xf zig-linux-x86_64-0.9.1.tar.xz
    RUN ln -s /earthbuild/zig-linux-x86_64-0.9.1/zig /bin/zig
    # zig builtins wasm tests
    RUN apt -y install build-essential
    # llvm
    RUN apt -y install lsb-release software-properties-common gnupg
    RUN wget https://apt.llvm.org/llvm.sh
    RUN chmod +x llvm.sh
    RUN ./llvm.sh 13
    RUN ln -s /usr/bin/clang-13 /usr/bin/clang
    # use lld as linker
    RUN ln -s /usr/bin/lld-13 /usr/bin/ld.lld
    ENV RUSTFLAGS="-C link-arg=-fuse-ld=lld -C target-cpu=native"
    RUN apt -y install libssl-dev
    RUN OPENSSL_NO_VENDOR=1 cargo install wasm-pack
    # sccache
    RUN cargo install sccache
    RUN sccache -V
    ENV RUSTC_WRAPPER=/usr/local/cargo/bin/sccache
    ENV SCCACHE_DIR=/earthbuild/sccache_dir
    ENV CARGO_INCREMENTAL=0 # no need to recompile package when using new function

copy-dirs:
    FROM +install-zig-llvm
    COPY --dir crates examples Cargo.toml Cargo.lock version.txt .cargo www rust-toolchain.toml ./

build-nightly-release:
    ARG RELEASE_FOLDER_NAME
    FROM +copy-dirs
    COPY --dir .git LICENSE LEGAL_DETAILS ci ./
    # version.txt is used by the CLI: roc --version
    RUN ./ci/write_version.sh
    RUN RUSTFLAGS="-C target-cpu=x86-64" cargo build --profile=release-with-lto --locked --bin roc
    # strip debug info
    RUN strip ./target/release-with-lto/roc
    RUN ./ci/package_release.sh $RELEASE_FOLDER_NAME
    RUN ls
    SAVE ARTIFACT ./$RELEASE_FOLDER_NAME.tar.gz AS LOCAL $RELEASE_FOLDER_NAME.tar.gz