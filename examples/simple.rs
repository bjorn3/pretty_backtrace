fn main() {
    pretty_backtrace::force_setup();
    std::thread::spawn(|| {
        assert!(std::panic::catch_unwind(|| please_panic(42)).is_err());
    }).join().unwrap();
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
    inner();
}

#[inline(always)]
fn inner() {
    let num = 41;
    pretty_backtrace::var_guard!(num);
    let _ = num;
    None::<()>.unwrap();
}
