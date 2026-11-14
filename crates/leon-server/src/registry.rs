//! Methods with their descriptions and JSON Schemas: one source of truth for `help.all`,
//! `leon api list/describe/call`, and (later) the Lua surface.

use leon_core::ApiError;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Serialize;
use serde_json::Value;

use crate::Ctx;

type Handler = Box<dyn Fn(&Ctx, Value) -> Result<Value, ApiError> + Send + Sync>;

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

fn schema<T: JsonSchema>() -> Value {
    serde_json::to_value(schemars::schema_for!(T)).expect("schemas serialize")
}

impl Registry {
    pub fn add<P, R>(&mut self, name: &'static str, description: &'static str, f: fn(&Ctx, P) -> Result<R, ApiError>)
    where
        P: DeserializeOwned + JsonSchema + 'static,
        R: Serialize + JsonSchema + 'static,
    {
        let handler = move |ctx: &Ctx, params: Value| {
            let p: P = serde_json::from_value(params).map_err(|e| ApiError::bad_request(format!("{name}: {e}")))?;
            Ok(serde_json::to_value(f(ctx, p)?).expect("results serialize"))
        };
        self.methods.push(Method {
            name,
            description,
            input_schema: schema::<P>(),
            output_schema: schema::<R>(),
            handler: Box::new(handler),
        });
    }

    pub fn methods(&self) -> &[Method] {
        &self.methods
    }

    /// Run a method. Missing (`null`) params mean `{}`.
    pub fn call(&self, ctx: &Ctx, name: &str, params: Value) -> Result<Value, ApiError> {
        let m = self.methods.iter().find(|m| m.name == name).ok_or_else(|| ApiError::method_not_found(name))?;
        let params = if params.is_null() { Value::Object(Default::default()) } else { params };
        (m.handler)(ctx, params)
    }
}
