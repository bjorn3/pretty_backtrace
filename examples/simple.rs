fn main() {
    pretty_backtrace::force_setup();
    assert!(std::panic::catch_unwind(|| please_panic(42)).is_err());
}

fn please_panic(num: u64) {
    pretty_backtrace::var_guard!(num);
    {
        let num2 = *num;
        pretty_backtrace::var_guard!(num2);
        let _ = num2;
    }
    let num3 = *num;
    pretty_backtrace::var_guard!(num3);
    let _ = num3;
    panic!("Some message");
}
