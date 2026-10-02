//! UI-independent primitives shared by every Synara runtime.
use chrono::{DateTime,Utc};use serde::{Deserialize,Serialize};use std::{fmt,str::FromStr};use uuid::Uuid;
#[derive(Clone,Copy,Debug,Eq,Hash,PartialEq,Serialize,Deserialize)]#[serde(transparent)]pub struct EntityId(Uuid);
impl EntityId{pub fn new()->Self{Self(Uuid::new_v4())}pub fn as_uuid(&self)->Uuid{self.0}}impl Default for EntityId{fn default()->Self{Self::new()}}impl fmt::Display for EntityId{fn fmt(&self,f:&mut fmt::Formatter<'_>)->fmt::Result{self.0.fmt(f)}}impl FromStr for EntityId{type Err=uuid::Error;fn from_str(s:&str)->Result<Self,Self::Err>{Ok(Self(Uuid::parse_str(s)?))}}
pub type Sequence=u64;pub type Revision=u32;pub type Timestamp=DateTime<Utc>;
#[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]pub struct Page<T>{pub items:Vec<T>,pub next_cursor:Option<String>}
#[derive(Clone,Debug,Eq,PartialEq,Serialize,Deserialize)]pub enum PolicyDecision{Allow,Deny,Ask}
#[derive(thiserror::Error,Debug)]pub enum CoreError{#[error("invalid identifier: {0}")]InvalidIdentifier(#[from]uuid::Error),#[error("validation failed: {0}")]Validation(String)}
