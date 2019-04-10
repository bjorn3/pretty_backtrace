# Pretty backtraces for rust

```rust
pretty_backtrace::setup();
panic!("Bomb!");
```

Pretty backtraces are normally only enabled when `RUST_BACKTRACE=pretty` to prevent breaking tools
which parse printed backtraces. If you want to always enable pretty backtraces use `force_setup`.

![screenshot](screenshot.png)
