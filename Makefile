TARGET ?= x86_64-unknown-linux-gnu
BIN = target/$(TARGET)/release/plex-downloader
CARGO ?= docker run -it --rm -v $(CURDIR):$(CURDIR) -w $(CURDIR) rust:latest cargo

default: $(BIN)

$(BIN): src/*.rs Cargo.*
	$(CARGO) build --release --target=$(TARGET)

scp: $(BIN)
	ssh plex@minty 'sudo systemctl stop plexdownloader'
	scp $< plex@minty:/var/lib/plexmediaserver
	ssh plex@minty 'sudo systemctl start plexdownloader'

test:
	cargo test

.PHONY: test
