//!   Copyright 2025 Isaac Schlaegel
//!
//!    Licensed under the Apache License, Version 2.0 (the "License");
//!    you may not use this file except in compliance with the License.
//!    You may obtain a copy of the License at
//!
//!        http://www.apache.org/licenses/LICENSE-2.0
//!
//!    Unless required by applicable law or agreed to in writing, software
//!    distributed under the License is distributed on an "AS IS" BASIS,
//!    WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
//!    See the License for the specific language governing permissions and
//!    limitations under the License.

use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use tracing::{
    Event,
    field::{Field, Visit},
    subscriber::Subscriber,
};
use tracing_subscriber::layer::{Context, Layer};

struct StringVisitor {
    msg: String,
}

impl Visit for StringVisitor {
    fn record_debug(&mut self, field: &Field, value: &dyn Debug) {
        self.msg
            .push_str(&format!("\"{}\":\"{:?}\"", field.name(), value))
    }
}

struct MemoryLayer {
    buffer: Arc<Mutex<Vec<String>>>,
}

impl<S: Subscriber> Layer<S> for MemoryLayer {
    fn on_event(&self, event: &Event<'_>, _ctx: Context<'_, S>) {
        let mut visitor = StringVisitor { msg: String::new() };
        event.record(&mut visitor);

        let mut buf = self.buffer.lock().unwrap();
        buf.push(visitor.msg);
    }
}
