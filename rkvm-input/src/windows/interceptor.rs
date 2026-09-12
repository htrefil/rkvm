use crate::interceptor::{InterceptorPlatform,Repeat};
use crate::abs::{AbsAxis, AbsInfo};
use crate::event::Event;
use crate::key::Key;
use crate::rel::RelAxis;


use std::collections::{HashMap, HashSet};
use std::ffi::CStr;
use std::io::Error;

pub struct InterceptorWindows;

impl InterceptorPlatform for InterceptorWindows {
    async fn read(&mut self) -> Result<Event, Error> {
        unimplemented!()
    }
    async fn write(&mut self, _event: &Event) -> Result<(), Error> {
        unimplemented!()
    }
    fn name(&self) -> &CStr {
        unimplemented!()
    }
    fn vendor(&self) -> u16 {
        unimplemented!()
    }
    fn product(&self) -> u16 {
        unimplemented!()
    }
    fn version(&self) -> u16 {
        unimplemented!()
    }
    fn rel(&self) -> HashSet<RelAxis> {
        unimplemented!()
    }
    fn abs(&self) -> HashMap<AbsAxis, AbsInfo> {
        unimplemented!()
    }
    fn key(&self) -> HashSet<Key> {
        unimplemented!()
    }
    fn repeat(&self) -> Repeat {
        unimplemented!()
    }
}