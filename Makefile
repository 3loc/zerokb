TARGET = armv7-unknown-linux-gnueabihf
INVENTORY = ansible/inventory.yml
PLAYBOOK = ansible/deploy-zerokb.yml

.PHONY: all build test check clean setup install upgrade

# Build and deploy
all: build
	ansible-playbook -i $(INVENTORY) $(PLAYBOOK)

build:
	cargo zigbuild --release --target $(TARGET) --bin zerokb
	@ls -lh target/$(TARGET)/release/zerokb

# First-time setup (prompts for sudo password, then configures passwordless sudo)
setup: build
	ansible-playbook -i $(INVENTORY) $(PLAYBOOK) --ask-become-pass

# Update all dependencies and rebuild everything
upgrade:
	cargo update
	$(MAKE) all
	ansible-playbook -i $(INVENTORY) ansible/upgrade.yml
	$(MAKE) install

test:
	cargo test

check:
	cargo check
	cargo clippy -- -D warnings

install:
	cargo install --path . --bin zerokb-tui

clean:
	cargo clean
