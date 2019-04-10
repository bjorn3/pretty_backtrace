# Pretty backtraces for rust

```rust
pretty_backtrace::setup();
panic!("Bomb!");
```

![screenshot](screenshot.png)

> This may break tools depending on a specific backtrace format. You may wish to only enable this
> when a certain env var is set:
>
> ```rust
> if std::env::var("PRETTY_BACKTRACE").is_ok() {
>     pretty_backtrace::setup();
> }
> ```
