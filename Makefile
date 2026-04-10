.PHONY: all linux windows clean

all: linux windows

linux:
	cargo build --release
	@echo "Linux binary: target/release/ant_simulation"

windows:
	cargo build --release --target x86_64-pc-windows-gnu
	@echo "Windows binary: target/x86_64-pc-windows-gnu/release/ant_simulation.exe"

clean:
	cargo clean
