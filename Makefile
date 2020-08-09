BIN = target/armv7-unknown-linux-musleabihf/release/plex-downloader
DOCKER ?= messense/rust-musl-cross:armv7-musleabihf
CARGO ?= docker run -it --rm \
	-v $(CURDIR):$(CURDIR) \
	-v ~/.cargo/git:/root/.cargo/git \
	-v ~/.cargo/registry:/root/.cargo/registry \
	-w $(CURDIR) $(DOCKER) cargo

default: $(BIN)

$(BIN): src/*.rs Cargo.*
	$(CARGO) build --release

scp: $(BIN)
	scp $< plex@helios:/var/lib/plexmediaserver

test:
	cargo test

.PHONY: test
