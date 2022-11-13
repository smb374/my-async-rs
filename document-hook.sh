#!/bin/sh
cargo doc --no-deps
rm -rf ./docs
echo "<meta http-equiv=\"refresh\" content=\"0; url=my_async\">" > target/doc/index.html
cp -r target/doc ./docs
