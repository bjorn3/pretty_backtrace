fn main() {
    pretty_backtrace::setup();
    please_panic(42);
}

fn please_panic(num: u64) {
    panic!("Some message");
}
