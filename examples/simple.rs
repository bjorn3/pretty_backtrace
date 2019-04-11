fn main() {
    pretty_backtrace::setup();
    please_panic(42);
}

fn please_panic(num: u64) {
    pretty_backtrace::backtrace_context!(num);

    panic!("Some message");
}
