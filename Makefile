BIN = target/$(TARGET)/release/plex-downloader
# DOCKER ?= rust:latest
DOCKER ?= dlecan/rust-crosscompiler-arm:stable
CARGO ?= docker run -it --rm \
	-v $(CURDIR):$(CURDIR) \
	-v ~/.cargo/git:/root/.cargo/git \
	-v ~/.cargo/registry:/root/.cargo/registry \
	-w $(CURDIR) $(DOCKER) cargo

USER ?= plex
SERVER ?= helios

default: $(BIN)

$(BIN): src/*.rs Cargo.*
	$(CARGO) build --release

scp: $(BIN)
	ssh $(USER)@$(SERVER) 'sudo systemctl stop plexdownloader'
	scp $< $(USER)@$(SERVER):/var/lib/plexmediaserver
	ssh $(USER)@$(SERVER) 'sudo systemctl start plexdownloader'

test:
	cargo test

.PHONY: test
