#!/bin/sh

cargo watch -s 'cargo doc'&
browser-sync start --ss ./target/doc -s ./target/doc -w --startPath "my_async/"
