//! `process()` из critters для опций, которые передаёт Next.js, как napi-аддон.
//! Какие документы сюда попадают, решает JS-сторона (src/core в корне пакета).

use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::LazyLock;

use napi::bindgen_prelude::*;
use napi_derive::napi;

use critters as pipeline;
use critters::FileCache;

static FILES: LazyLock<FileCache> = LazyLock::new(|| FileCache::new(true));

#[napi(object)]
pub struct ProcessOptions {
    pub path: String,
    pub public_path: String,
}

#[napi(object)]
pub struct LogEntry {
    /// `info` | `warn`
    pub level: String,
    pub message: String,
}

#[napi(object)]
pub struct ProcessResult {
    /// Обработанный документ; отсутствует, когда вместо него должна отработать JS-реализация.
    pub html: Option<String>,
    pub logs: Vec<LogEntry>,
    pub fallback: Option<String>,
}

fn run(html: &str, options: &ProcessOptions) -> ProcessResult {
    let options = pipeline::Options {
        path: &options.path,
        public_path: &options.public_path,
    };
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        pipeline::process(html, &options, &FILES)
    }))
    .unwrap_or_else(|_| pipeline::Outcome::Fallback("panic".to_owned()));
    match outcome {
        pipeline::Outcome::Html { html, logs } => ProcessResult {
            html: Some(html),
            logs: logs
                .into_iter()
                .map(|log| LogEntry {
                    level: match log.level {
                        pipeline::Level::Info => "info",
                        pipeline::Level::Warn => "warn",
                    }
                    .to_owned(),
                    message: log.message,
                })
                .collect(),
            fallback: None,
        },
        pipeline::Outcome::Fallback(reason) => ProcessResult {
            html: None,
            logs: Vec::new(),
            fallback: Some(reason),
        },
    }
}

pub struct ProcessTask {
    html: String,
    options: ProcessOptions,
}

impl Task for ProcessTask {
    type Output = ProcessResult;
    type JsValue = ProcessResult;

    fn compute(&mut self) -> Result<Self::Output> {
        Ok(run(&self.html, &self.options))
    }

    fn resolve(&mut self, _env: Env, output: Self::Output) -> Result<Self::JsValue> {
        Ok(output)
    }
}

/// Выполняется в пуле потоков libuv, вне event loop.
#[napi]
pub fn process(html: String, options: ProcessOptions) -> AsyncTask<ProcessTask> {
    AsyncTask::new(ProcessTask { html, options })
}
