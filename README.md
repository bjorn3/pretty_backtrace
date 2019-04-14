# Pretty backtraces for rust

[![Cargo](https://img.shields.io/crates/v/pretty_backtrace.svg)](https://crates.io/crates/pretty_backtrace)
[![Build Status](https://travis-ci.com/bjorn3/pretty_backtrace.svg?branch=master)](https://travis-ci.com/bjorn3/pretty_backtrace)

```rust
pretty_backtrace::setup();
panic!("Bomb!");
```

Pretty backtraces are normally only enabled when `RUST_BACKTRACE=pretty` to prevent breaking tools
which parse printed backtraces. If you want to always enable pretty backtraces use `force_setup`.

![screenshot](screenshot.png)
