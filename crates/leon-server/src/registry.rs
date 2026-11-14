//! Methods with their descriptions and JSON Schemas: one source of truth for `help.all`,
//! `leon api list/describe/call`, and (later) the Lua surface.
//!
//! Every handler returns a future. Most methods finish at once and return a ready one; a method
//! that waits (`block.wait`) does its checks now and hands back a future for the rest, so a
//! pending call never holds a server thread.

use std::future::{ready, Future};
use std::pin::Pin;
use std::sync::Arc;

use leon_core::ApiError;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::Ctx;

/// A reply that may arrive later.
pub type Later<T> = Pin<Box<dyn Future<Output = Result<T, ApiError>> + Send>>;

type Handler = Box<dyn Fn(&Arc<Ctx>, Value) -> Later<Value> + Send + Sync>;

pub struct Method {
    pub name: &'static str,
    pub description: &'static str,
    pub input_schema: Value,
    pub output_schema: Value,
    handler: Handler,
}

#[derive(Default)]
pub struct Registry {
    methods: Vec<Method>,
}

pub(crate) fn schema<T: JsonSchema>() -> Value {
    serde_json::to_value(schemars::schema_for!(T)).expect("schemas serialize")
}

impl Registry {
    /// A method that replies at once.
    pub fn add<P, R>(
        &mut self,
        name: &'static str,
        description: &'static str,
        f: fn(&Arc<Ctx>, P) -> Result<R, ApiError>,
    ) where
        P: DeserializeOwned + JsonSchema + 'static,
        R: Serialize + JsonSchema + 'static,
    {
        let handler = move |ctx: &Arc<Ctx>, params: Value| -> Later<Value> {
            Box::pin(ready(parse(name, params).and_then(|p| f(ctx, p)).map(to_value)))
        };
        self.push::<P, R>(name, description, Box::new(handler));
    }

    /// A method whose reply may come later: `f` runs at once and returns a future for the rest,
    /// which must not block (await instead).
    pub fn add_deferred<P, R>(
        &mut self,
        name: &'static str,
        description: &'static str,
        f: fn(&Arc<Ctx>, P) -> Result<Later<R>, ApiError>,
    ) where
        P: DeserializeOwned + JsonSchema + 'static,
        R: Serialize + JsonSchema + 'static,
    {
        let handler = move |ctx: &Arc<Ctx>, params: Value| -> Later<Value> {
            match parse(name, params).and_then(|p| f(ctx, p)) {
                Ok(later) => Box::pin(async move { later.await.map(to_value) }),
                Err(e) => Box::pin(ready(Err(e))),
            }
        };
        self.push::<P, R>(name, description, Box::new(handler));
    }

    fn push<P: JsonSchema, R: JsonSchema>(&mut self, name: &'static str, description: &'static str, handler: Handler) {
        self.methods.push(Method {
            name,
            description,
            input_schema: schema::<P>(),
            output_schema: schema::<R>(),
            handler,
        });
    }

    pub fn methods(&self) -> &[Method] {
        &self.methods
    }

    /// Run a method. Missing (`null`) params mean `{}`.
    pub fn call(&self, ctx: &Arc<Ctx>, name: &str, params: Value) -> Later<Value> {
        let Some(m) = self.methods.iter().find(|m| m.name == name) else {
            return Box::pin(ready(Err(ApiError::method_not_found(name))));
        };
        let params = if params.is_null() { Value::Object(Default::default()) } else { params };
        (m.handler)(ctx, params)
    }
}

pub(crate) fn parse<P: DeserializeOwned>(name: &str, params: Value) -> Result<P, ApiError> {
    serde_json::from_value(params).map_err(|e| ApiError::bad_request(format!("{name}: {e}")))
}

pub(crate) fn to_value<R: Serialize>(r: R) -> Value {
    serde_json::to_value(r).expect("results serialize")
}
