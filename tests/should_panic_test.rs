#[test]
#[should_panic]
fn panic() {
    pretty_backtrace::force_setup();
    panic!("boom")
}
