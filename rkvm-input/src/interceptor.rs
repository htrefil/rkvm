use crate::abs::{AbsAxis, AbsInfo};
use crate::event::Event;
use crate::key::Key;
use crate::rel::RelAxis;


use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::io::Error;

pub struct Repeat {
    pub delay: Option<i32>,
    pub period: Option<i32>,
}

pub trait InterceptorPlatform: Sized {
    fn read<'a>(&'a mut self) -> impl std::future::Future<Output = Result<Event, Error>> + Send + 'a;
    fn write<'a>(&'a mut self, event: &'a Event) -> impl std::future::Future<Output = Result<(), Error>> + Send + 'a;
    
    fn name(&self) -> &CStr;

    fn vendor(&self) -> u16;

    fn product(&self) -> u16;

    fn version(&self) -> u16;

    fn rel(&self) ->  HashSet<RelAxis>;

    fn abs(&self) -> HashMap<AbsAxis, AbsInfo>;

    fn key(&self) -> HashSet<Key>;

    fn repeat(&self) -> Repeat;
}
