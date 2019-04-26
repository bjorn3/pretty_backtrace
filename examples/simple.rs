fn main() {
    pretty_backtrace::setup();
    please_panic(Enum::Num(42));
}

fn please_panic(_num: Enum) {
    panic!("Some message");
}

enum Enum {
    Num(u64),
    /**/Val,
}
