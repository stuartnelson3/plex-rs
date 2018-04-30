TARGET ?= x86_64-unknown-linux-musl
BIN = target/$(TARGET)/release/plex-downloader
CARGO ?= docker run -it --rm -v $(CURDIR):$(CURDIR) -w $(CURDIR) rust:latest cargo

default: $(BIN)

$(BIN): src/*.rs
	$(CARGO) build --release --target=$(TARGET)

scp: $(BIN)
	scp $< plex@minty:/var/lib/plexmediaserver

test:
	cargo test

.PHONY: test
