# my-async-rs

An example asynchronous runtime in Rust.

This project is primarily served as the codebase of my Graduation Project,
_Analysis and Research on Rust's Asynchronous I/O Runtime Approach_, advised by
[Prof. Chun-Ying Huang](https://people.cs.nycu.edu.tw/~chuang/) at National
Yang Ming Chiao Tung University for my BSc degree in Computer Science during
March 2022 to January 2023.

## Purpose

The purpose of this project is to serve as an easy source for people to
understand what an implementation of a async runtime in Rust could look like
before looking at much more complex codebases like those of
[tokio](https://github.com/tokio-rs/tokio) and
[async-std](https://github.com/async-rs/async-std).
The project also provides a design document to demonstrate my design approach on
details, which can be found at the end at the [Documents] section.

## Overview

- The code contains a single-threaded runtime and a multi-threaded runtime with
3 different work scheduler implementations.
- The multi-threaded runtime uses message passing to control the underlying
scheduler and [mio](https://github.com/tokio-rs/mio) as the reactor to receive
IO events from the OS.
- The scheduler of the multi-threaded runtime is defined as a trait to be able
to contain multiple implementation. Currently, round-robin, work stealing, and a
hybrid strategy that combines work stealing and priority queue are implemented.

## Documents

Design Documents can be found at [Here](https://smb374.github.io/my-async-rs/design/).

<!-- vim: set colorcolumn=80 textwidth=80: -->
