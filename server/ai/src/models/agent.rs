use std::marker::PhantomData;

use rig::agent::AgentBuilder;
use schemars::JsonSchema;

pub struct ModelAgent {
    pub agent: rig::agent::Agent,
    pub model: String,
}

pub struct StructuredAgent<T> {
    pub agent: ModelAgent,
    answer: PhantomData<fn() -> T>,
}

impl<T: JsonSchema> StructuredAgent<T> {
    pub fn new(builder: AgentBuilder, model: String) -> Self {
        Self {
            agent: ModelAgent {
                agent: builder.output_schema::<T>().build(),
                model,
            },
            answer: PhantomData,
        }
    }
}
