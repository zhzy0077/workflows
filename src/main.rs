mod command;
mod decompress;
mod download;
mod echo;
mod gist;
mod http;
mod parser;
mod util;
mod wechat;

use crate::command::Command;
use crate::decompress::Decompress;
use crate::download::Download;
use crate::echo::Echo;
use crate::gist::Gist;
use crate::http::Http;
use crate::wechat::WeChat;
use anyhow::{anyhow, Context as _, Result};
use enum_dispatch::enum_dispatch;
use lazy_static::lazy_static;
use parser::fulfill;
use serde::Deserialize;
use std::{collections::HashMap, env, fs};

const USER_AGENT: &'static str = "workflows/1.0";

#[enum_dispatch(SupportedWorkflows)]
trait Workflow {
    fn execute(&self, input: Payload) -> Result<Payload>;
    fn parameters(&self) -> &'static [&'static str];
    fn outputs(&self) -> &'static [&'static str];
}

#[derive(Debug)]
pub struct Context {
    env: HashMap<String, String>,
    input: HashMap<String, String>,
}

impl Context {
    fn new() -> Self {
        let env: HashMap<String, String> = env::vars().collect::<_>();
        let input: HashMap<String, String> = HashMap::new();

        Self { env, input }
    }
}

#[derive(Debug)]
struct Payload {
    parameters: HashMap<&'static str, String>,
}

impl Payload {
    fn new(parameters: HashMap<&'static str, String>) -> Self {
        Self { parameters }
    }

    fn parameter(&self, key: &'static str) -> &str {
        self.parameters
            .get(key)
            .map(|s| &s[..])
            .unwrap_or_else(|| "")
    }
}

#[enum_dispatch]
enum SupportedWorkflows {
    Http,
    Gist,
    Echo,
    WeChat,
    Command,
    Download,
    Decompress,
}

lazy_static! {
    static ref WORKFLOWS: HashMap<&'static str, SupportedWorkflows> = {
        let mut m = HashMap::new();
        m.insert("http", Http {}.into());
        m.insert("echo", Echo {}.into());
        m.insert("wechat", WeChat {}.into());
        m.insert("gist", Gist {}.into());
        m.insert("command", Command {}.into());
        m.insert("download", Download {}.into());
        m.insert("decompress", Decompress {}.into());
        m
    };
}

#[derive(Debug, Deserialize)]
struct Config {
    workflows: Vec<WorkflowConfig>,
}
#[derive(Debug, Deserialize)]
struct WorkflowConfig {
    #[serde(rename = "type")]
    workflow_type: String,
    parameters: HashMap<String, String>,
}

fn make_workflow(
    config: &WorkflowConfig,
    context: &Context,
) -> Result<(&'static SupportedWorkflows, Payload)> {
    let workflow = WORKFLOWS
        .get(&config.workflow_type.to_lowercase()[..])
        .context(anyhow!("Workflow {} is not found.", config.workflow_type))?;
    let mut payload: HashMap<&'static str, String> = HashMap::new();
    for key in workflow.parameters() {
        if let Some(value) = config.parameters.get(*key) {
            payload.insert(key, fulfill(value, &context)?);
        }
    }
    Ok((workflow, Payload::new(payload)))
}

fn main() -> Result<()> {
    let config_path = env::args()
        .nth(1)
        .context("No configuration is provided.")?;

    let config = fs::read_to_string(config_path)?;
    let config: Config = serde_yaml::from_str(&config)?;

    let mut context = Context::new();
    for workflow_config in config.workflows.into_iter() {
        let (workflow, payload) = make_workflow(&workflow_config, &context)?;
        let output = workflow.execute(payload)?;

        context.input = output
            .parameters
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect();
    }

    Ok(())
}
