mod process;
mod errors;
mod event;

use regex::Regex;
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Duration;

pub use process::{Process, ProcessContext};

pub use errors::Error;
pub use event::{TerraformEvent, TerraformResourceChange, TerraformResourceStatus, TerraformSourceStream};

pub struct Terraform<P, Q>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    pub process: Process<P, Q>,
    pub sender: Sender<TerraformEvent>,
    plan_change_regex: Regex,
    pre_apply_regex: Regex,
    still_applying_regex: Regex,
    post_apply_regex: Regex,
    plan_completed_regex: Regex,
    apply_completed_regex: Regex,
    destroy_completed_regex: Regex,
}

impl<P, Q> Terraform<P, Q>
where
    P: AsRef<Path>,
    Q: AsRef<Path>,
{
    pub fn new(
        binary_path: P,
        working_directory: Q,
        envs: HashMap<String, String>,
        timeout: Duration,
        sender: Sender<TerraformEvent>,
    ) -> Result<Self, Error> {
        let process = Process::new(binary_path, working_directory, envs, timeout);

        Ok(Self {
            process,
            sender,
            // "  # %s will be created"
            // "  # %s will be read during apply"
            // "  # %s will be updated in-place"
            // "  # %s will be destroyed"
            // "  # %s is tainted, so must be replaced"
            // "  # %s must be replaced"
            plan_change_regex: Regex::new(
                "  # (?P<address>.+) ((will be ((?P<action_create>created)|((?P<action_read>read) during apply)|((?P<action_update>updated) in-place)|(?P<action_destroy>destroyed)))|((is tainted, so )?must be (?P<action_replace>replaced)))"
            )?,
            // "(addr)( \(generation\))?: (Destroying|Creating|Modifying|Reading)...( [key=value])?"
            pre_apply_regex: Regex::new(
                r"^(?P<address>.+)( \((?P<generation>.*)\))?: (?P<action>(Destroying|Creating|Modifying|Reading))\.\.\.(?: \[(?P<id_key>.+)=(?P<id_value>.+)\])?$",
            )?,
            // "(addr): Still (modifying|destroying|creating|reading)... [(key=value, )?(elapsed)]"
            still_applying_regex: Regex::new(
                r"^(?P<address>.+): Still (?P<action>(modifying|destroying|creating|reading))\.\.\. \[(?:(?P<id_key>.+)=(?P<id_value>.+), )?(?P<elapsed>\d+\w+) elapsed\]",
            )?,
            // "(addr): (Modifications|Destruction|Creation|Read) complete after (elapsed)( [key=value])?"
            post_apply_regex: Regex::new(
                r"^(?P<address>.+): (?P<action>(Modifications|Destruction|Creation|Read)) complete after (?P<elapsed>\d+\w+)(?: \[(?P<id_key>.+)=(?P<id_value>.+)\])?$",
            )?,

            plan_completed_regex: Regex::new(r"Plan: (?P<add_count>\d)+ to add, (?P<change_count>\d)+ to change, (?P<destroy_count>\d)+ to destroy.")?,
            apply_completed_regex: Regex::new(r"Apply complete! Resources: (?P<add_count>\d)+ added, (?P<change_count>\d)+ changed, (?P<destroy_count>\d)+ destroyed.")?,
            destroy_completed_regex: Regex::new(r"Destroy complete! Resources: (?P<destroy_count>\d)+ destroyed.")?,
        })
    }

    pub fn run_init(&self) -> Result<ProcessContext, Error> {
        let command: &str = "init";

        Ok(self.process.spawn(vec!["init", "-force-copy", "-no-color"])?.wait(
            |stdout| {
                if let Some(stdout) = stdout {
                    let _ = self.sender.send(TerraformEvent {
                        command: String::from(command),
                        source: stdout,
                        source_stream: TerraformSourceStream::Stdout,
                        ..TerraformEvent::default()
                    });
                }
            },
            |stderr| {
                if let Some(stderr) = stderr {
                    let _ = self.sender.send(TerraformEvent {
                        command: String::from(command),
                        source: stderr,
                        source_stream: TerraformSourceStream::Stderr,
                        ..TerraformEvent::default()
                    });
                }
            },
        )?)
    }

    pub fn run_plan(&self, target_plan: P) -> Result<ProcessContext, Error>
    where
        P: AsRef<Path>,
    {
        let command: &str = "plan";
        let plan_path = target_plan.as_ref().to_str().ok_or(Error::PathError)?;
        let out_arg = format!("-out={}", plan_path);

        Ok(self.process.spawn(vec!["plan", "-input=false", out_arg.as_ref(), "-no-color"])?.wait(
            |stdout| {
                if let Some(stdout) = stdout {
                    let _ = self.sender.send(TerraformEvent {
                        command: String::from(command),
                        ..self.parse_plan_stdout(stdout)
                    });
                }
            },
            |stderr| {
                if let Some(stderr) = stderr {
                    let _ = self.sender.send(TerraformEvent {
                        command: String::from(command),
                        source: stderr,
                        source_stream: TerraformSourceStream::Stderr,
                        ..TerraformEvent::default()
                    });
                }
            },
        )?)
    }

    pub fn run_apply(&self, target_plan: P) -> Result<ProcessContext, Error>
    where
        P: AsRef<Path>,
    {
        let command: &str = "apply";
        let plan_path = target_plan.as_ref().to_str().ok_or(Error::PathError)?;

        Ok(self
            .process
            .spawn(vec!["apply", "-auto-approve", "-input=false", "-no-color", plan_path])?
            .wait(
                |stdout| {
                    if let Some(stdout) = stdout {
                        let _ = self.sender.send(TerraformEvent {
                            command: String::from(command),
                            ..self.parse_apply_stdout(stdout)
                        });
                    }
                },
                |stderr| {
                    if let Some(stderr) = stderr {
                        let _ = self.sender.send(TerraformEvent {
                            command: String::from(command),
                            source: stderr,
                            source_stream: TerraformSourceStream::Stderr,
                            ..TerraformEvent::default()
                        });
                    }
                },
            )?)
    }

    pub fn run_destroy(&self) -> Result<ProcessContext, Error> {
        let command: &str = "destroy";

        Ok(self
            .process
            .spawn(vec!["destroy", "-auto-approve", "-no-color"])?
            .wait(
                |stdout| {
                    if let Some(stdout) = stdout {
                        let _ = self.sender.send(TerraformEvent {
                            command: String::from(command),
                            ..self.parse_apply_stdout(stdout)
                        });
                    }
                },
                |stderr| {
                    if let Some(stderr) = stderr {
                        let _ = self.sender.send(TerraformEvent {
                            command: String::from(command),
                            source: stderr,
                            source_stream: TerraformSourceStream::Stderr,
                            ..TerraformEvent::default()
                        });
                    }
                },
            )?)
    }

    fn parse_plan_stdout(&self, stdout: String) -> TerraformEvent {
        if let Some(captures) = self.plan_change_regex.clone().captures(stdout.as_str()) {
            let (address, _, _) = self.parse_context_captures(&captures);

            TerraformEvent {
                change: self.captures_to_change(captures),
                status: Some(TerraformResourceStatus::Planned),
                resource_path: address,
                source: stdout,
                ..TerraformEvent::default()
            }
        } else if let Some(captures) = self.plan_completed_regex.clone().captures(stdout.as_str()) {
            let (create_count, update_count, delete_count) = self.parse_stats_captures(&captures);

            TerraformEvent {
                status: Some(TerraformResourceStatus::Completed),
                source: stdout,
                create_count,
                update_count,
                delete_count,
                ..TerraformEvent::default()
            }
        } else {
            TerraformEvent {
                status: Some(TerraformResourceStatus::Planned),
                source: stdout,
                ..TerraformEvent::default()
            }
        }
    }

    fn parse_apply_stdout(&self, stdout: String) -> TerraformEvent {
        if let Some(captures) = self.pre_apply_regex.clone().captures(stdout.as_str()) {
            let (address, id_key, id_value) = self.parse_context_captures(&captures);

            TerraformEvent {
                change: self.captures_to_change(captures),
                status: Some(TerraformResourceStatus::Started),
                resource_path: address,
                id_key,
                id_value,
                source: stdout,
                ..TerraformEvent::default()
            }
        } else if let Some(captures) = self.still_applying_regex.clone().captures(stdout.as_str()) {
            let (address, id_key, id_value) = self.parse_context_captures(&captures);

            TerraformEvent {
                change: self.captures_to_change(captures),
                status: Some(TerraformResourceStatus::InProgress),
                resource_path: address,
                id_key,
                id_value,
                source: stdout,
                ..TerraformEvent::default()
            }
        } else if let Some(captures) = self.post_apply_regex.clone().captures(stdout.as_str()) {
            let (address, id_key, id_value) = self.parse_context_captures(&captures);

            TerraformEvent {
                change: self.captures_to_change(captures),
                status: Some(TerraformResourceStatus::Done),
                resource_path: address,
                id_key,
                id_value,
                source: stdout,
                ..TerraformEvent::default()
            }
        } else if let Some(captures) = self.apply_completed_regex.clone().captures(stdout.as_str()) {
            let (create_count, update_count, delete_count) = self.parse_stats_captures(&captures);

            TerraformEvent {
                status: Some(TerraformResourceStatus::Completed),
                source: stdout,
                create_count,
                update_count,
                delete_count,
                ..TerraformEvent::default()
            }
        } else if let Some(captures) = self.destroy_completed_regex.clone().captures(stdout.as_str()) {
            let (create_count, update_count, delete_count) = self.parse_stats_captures(&captures);

            TerraformEvent {
                status: Some(TerraformResourceStatus::Completed),
                source: stdout,
                create_count,
                update_count,
                delete_count,
                ..TerraformEvent::default()
            }
        } else {
            TerraformEvent {
                source: stdout,
                ..TerraformEvent::default()
            }
        }
    }

    fn parse_stats_captures(&self, captures: &regex::Captures) -> (Option<u32>, Option<u32>, Option<u32>) {
        (
            captures
                .name("add_count")
                .map(|m| String::from(m.as_str().trim()).parse::<u32>().ok())
                .flatten(),
            captures
                .name("change_count")
                .map(|m| String::from(m.as_str().trim()).parse::<u32>().ok())
                .flatten(),
            captures
                .name("destroy_count")
                .map(|m| String::from(m.as_str().trim()).parse::<u32>().ok())
                .flatten(),
        )
    }

    fn parse_context_captures(&self, captures: &regex::Captures) -> (Option<String>, Option<String>, Option<String>) {
        (
            captures.name("address").map(|m| String::from(m.as_str().trim())),
            captures.name("id_key").map(|m| String::from(m.as_str().trim())),
            captures.name("id_value").map(|m| String::from(m.as_str().trim())),
        )
    }

    fn captures_to_change(&self, captures: regex::Captures) -> Vec<TerraformResourceChange> {
        if let Some(change) = captures.name("action") {
            self.action_to_change(change.as_str())
        } else if captures.name("action_create").is_some() {
            vec![TerraformResourceChange::Create]
        } else if captures.name("action_read").is_some() {
            vec![TerraformResourceChange::Read]
        } else if captures.name("action_update").is_some() {
            vec![TerraformResourceChange::Update]
        } else if captures.name("action_destroy").is_some() {
            vec![TerraformResourceChange::Destroy]
        } else if captures.name("action_replace").is_some() {
            vec![TerraformResourceChange::Destroy, TerraformResourceChange::Create]
        } else {
            Vec::new()
        }
    }

    fn action_to_change(&self, action: &str) -> Vec<TerraformResourceChange> {
        match action.trim() {
            "Creating" | "creating" | "Creation" => vec![TerraformResourceChange::Create],
            "Reading" | "reading" | "Read" => vec![TerraformResourceChange::Read],
            "Modifying" | "modifying" | "Modifications" => vec![TerraformResourceChange::Update],
            "Destroying" | "destroying" | "Destruction" => vec![TerraformResourceChange::Destroy],
            _ => Vec::new(),
        }
    }
}
