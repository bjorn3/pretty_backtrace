use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use std::ops::{Deref, DerefMut, Drop};
use std::marker::PhantomData;

use findshlibs::Avma;

pub struct BacktraceContext<T: Debug + 'static>(Rc<BacktraceContextInner>, PhantomData<T>);

struct BacktraceContextInner(&'static str, Box<BacktraceContextValue>);

trait BacktraceContextValue: mopa::Any + Debug {
    fn as_debug(&self) -> &dyn Debug;
}

mopafy!(BacktraceContextValue);

impl<T: mopa::Any + Debug> BacktraceContextValue for T {
    fn as_debug(&self) -> &dyn Debug {
        self
    }
}

impl<T: Debug + 'static> Deref for BacktraceContext<T> {
    type Target = T;

    fn deref(&self) -> &T {
        (self.0).1.downcast_ref().unwrap()
    }
}

/*impl<T: Debug + 'static> DerefMut for BacktraceContext<T> {
    fn deref_mut(&mut self) -> &mut T {
        (self.0).1.downcast_mut().unwrap()
    }
}*/

impl<T: Debug + 'static> BacktraceContext<T> {
    pub fn new(name: &'static str, val: T) -> Self {
        let inner = Rc::new(BacktraceContextInner(name, Box::new(val)));
        CONTEXTS.with(|contexts| {
            let mut contexts = contexts.borrow_mut();
            contexts.push((Avma(0 as *const u8), inner.clone()));
        });
        Self(inner, PhantomData)
    }

    //pub fn into_inner(self: Self) -> T {
    //    *Rc::try_unwrap(self.0).unwrap_or_else(|_| panic!("there should only be one remaining instance of this")).1.downcast().unwrap()
    //}
}

impl<T: Debug> Drop for BacktraceContext<T> {
    fn drop(&mut self) {
        CONTEXTS.with(|contexts| {
            let mut contexts = contexts.borrow_mut();
            let (_avma, context) = contexts.pop().unwrap();
            assert!(Rc::ptr_eq(&self.0, &context));
        });
    }
}

#[macro_export]
macro_rules! backtrace_context {
    (let $var:ident = $val:expr) => {
        let $var = $crate::BacktraceContext::new(stringify!($var), $val);
    };

    ($val:expr) => {
        let _a = $crate::BacktraceContext::new(stringify!($val), $val);
    }
}

thread_local! {
    static CONTEXTS: RefCell<Vec<(Avma, Rc<BacktraceContextInner>)>> = RefCell::new(Vec::new());
}

pub fn print_context() {
    CONTEXTS.with(|contexts| {
        let contexts = contexts.borrow();
        for (avma, context) in &*contexts {
            println!("{:?}: {} = {:?}", avma, context.0, context.1.as_debug());
        }
    })
}
