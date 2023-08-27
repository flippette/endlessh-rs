start-test-dev: stop-test
	cargo build
	PORT=2200 target/debug/endlessh-rs &
	just -f {{justfile()}} stress

start-test-rel: stop-test
	cargo build --release
	PORT=2200 target/release/endlessh-rs &
	just -f {{justfile()}} stress

stress:
	for i in $(seq 1 3000); do sleep 0.5s && ssh localhost -p 2200 -v 1>/dev/null 2>&1 & disown; done
	
stop-test:
	-kill $(pgrep endlessh-rs)
