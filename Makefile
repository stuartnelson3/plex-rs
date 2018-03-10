TARGET ?= x86_64-unknown-linux-musl
BIN = target/$(TARGET)/release/plex-downloader

default: $(BIN)

$(BIN): src/*.rs
	cargo build --release --target=$(TARGET)

scp: $(BIN)
	scp $< plex@minty:/var/lib/plexmediaserver

test:
	cargo test

.PHONY: test
