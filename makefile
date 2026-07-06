BINARY=nibble
PREFIX=/usr/local
BINDIR=$(PREFIX)/bin

.PHONY: all build install uninstall clean

all: build

build:
	cargo build --release

# run with root/sudo privilages
install:
	install -m 755 ./target/release/nibble $(BINDIR)/$(BINARY)

uninstall:
	rm -f $(BINDIR)/$(BINARY)

clean:
	cargo clean