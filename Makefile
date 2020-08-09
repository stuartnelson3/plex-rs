BIN = target/$(TARGET)/release/plex-downloader
DOCKER ?= stuartnelson3/rust-cross-compiler-arm
CARGO ?= docker run -it --rm \
	-v $(CURDIR):$(CURDIR) \
	-v ~/.cargo/git:/cargo/git \
	-v ~/.cargo/registry:/cargo/registry \
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
