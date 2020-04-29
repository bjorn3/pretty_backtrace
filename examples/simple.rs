fn main() {
    pretty_backtrace::force_setup();
    assert!(std::panic::catch_unwind(|| please_panic(42)).is_err());
}

fn please_panic(_num: u64) {
    panic!("Some message");
}
