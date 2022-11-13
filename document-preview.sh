#!/bin/sh

cargo watch -s 'cargo doc --no-deps'&
browser-sync start --ss ./target/doc -s ./target/doc -w --startPath "my_async/"
