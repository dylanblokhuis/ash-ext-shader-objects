use std::borrow::Cow;

use crate::ctx::ExampleBase;

pub trait RenderNode: Send + Sync + Sized {
    unsafe fn run(&self, base: &ExampleBase)
    where
        Self: Sized;
}
