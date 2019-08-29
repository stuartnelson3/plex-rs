BIN = target/$(TARGET)/release/plex-downloader
# DOCKER ?= rust:latest
DOCKER ?= dlecan/rust-crosscompiler-arm:stable
CARGO ?= docker run -it --rm -v $(CURDIR):$(CURDIR) -w $(CURDIR) $(DOCKER) cargo

default: $(BIN)

$(BIN): src/*.rs Cargo.*
	$(CARGO) build --release

scp: $(BIN)
	ssh plex@minty 'sudo systemctl stop plexdownloader'
	scp $< plex@minty:/var/lib/plexmediaserver
	ssh plex@minty 'sudo systemctl start plexdownloader'

test:
	cargo test

.PHONY: test
